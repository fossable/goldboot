use crate::library::ImageLibrary;
use crate::registry::protocol::RegistryImageEntry;
use crate::{can_preload, gpt::fixup_backup_gpt, registry::Client};
use goldboot_image::ImageHandle;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tracing::{debug, warn};

/// What the user picked on the SelectImage screen.
#[derive(Debug, Clone)]
pub enum SelectedImage {
    /// An image in the local library, identified by its SHA256 id.
    Local(String),
    /// An image on a remote registry, identified by `host`, `name`, `tag`.
    Registry {
        host: String,
        name: String,
        tag: String,
    },
}

#[cfg(feature = "uki")]
pub struct DebugShell {
    pub terminal_backend: egui_term::TerminalBackend,
    pub pty_event_receiver: std::sync::mpsc::Receiver<(u64, egui_term::PtyEvent)>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BlockState {
    /// Not yet processed
    Pending,
    /// Currently being written
    Writing,
    /// Block was already correct, no write needed
    UpToDate,
    /// Block was dirty and has been written
    Written,
    /// Currently being read and hashed
    Verifying,
    /// Hash matched
    Verified,
    /// Hash did not match (corruption)
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PostWriteDialog {
    /// Waiting for user choice
    Visible,
    /// Dialog dismissed (verifying or done)
    Hidden,
}

pub struct WriteProgress {
    /// Total number of clusters
    pub cluster_count: usize,
    /// Bytes per cluster
    pub block_size: u64,
    /// Per-cluster state
    pub block_states: Vec<BlockState>,
    /// Write (or verify) thread has finished
    pub done: bool,
    /// Verification pass is running
    pub verifying: bool,
    /// True if this is a verify-only operation (no write)
    pub verify_only: bool,
    /// Dialog state after operation completes
    pub post_write_dialog: Option<PostWriteDialog>,
    /// Set if write or verify failed
    pub error: Option<String>,
    /// When the operation started
    pub start_time: Instant,
    /// Final elapsed time when operation completes
    pub elapsed_final: Option<f64>,

    // Speed tracking
    bytes_read_total: u64,
    bytes_written_total: u64,
    last_sample_time: Instant,
    last_bytes_read: u64,
    last_bytes_written: u64,
    /// Read speed in bytes/sec, updated each cluster
    pub read_speed: f64,
    /// Write speed in bytes/sec, updated each cluster
    pub write_speed: f64,
}

impl WriteProgress {
    pub fn new(cluster_count: usize, block_size: u64) -> Self {
        let now = Instant::now();
        Self {
            cluster_count,
            block_size,
            block_states: vec![BlockState::Pending; cluster_count],
            done: false,
            verifying: false,
            verify_only: false,
            post_write_dialog: None,
            error: None,
            start_time: now,
            elapsed_final: None,
            bytes_read_total: 0,
            bytes_written_total: 0,
            last_sample_time: now,
            last_bytes_read: 0,
            last_bytes_written: 0,
            read_speed: 0.0,
            write_speed: 0.0,
        }
    }

    /// Called by the write thread for each cluster event.
    /// `state` matches the `ImageHandle::write` callback convention:
    /// - `None`        — cluster is dirty and is now being written
    /// - `Some(true)`  — cluster has been written
    /// - `Some(false)` — cluster was already up to date
    pub fn record_cluster(&mut self, idx: usize, state: Option<bool>) {
        self.block_states[idx] = match state {
            None => BlockState::Writing,
            Some(true) => BlockState::Written,
            Some(false) => BlockState::UpToDate,
        };

        // Only update byte counters and speeds on completion events, not on Writing signal
        if let Some(was_dirty) = state {
            self.bytes_read_total += self.block_size;
            if was_dirty {
                self.bytes_written_total += self.block_size;
            }

            // Update speed estimate every ~0.25 s
            let elapsed = self.last_sample_time.elapsed().as_secs_f64();
            if elapsed >= 0.25 {
                self.read_speed = (self.bytes_read_total - self.last_bytes_read) as f64 / elapsed;
                self.write_speed =
                    (self.bytes_written_total - self.last_bytes_written) as f64 / elapsed;
                self.last_sample_time = Instant::now();
                self.last_bytes_read = self.bytes_read_total;
                self.last_bytes_written = self.bytes_written_total;
            }
        }
    }

    /// Called by the verify thread for each cluster event.
    /// `state` matches the `ImageHandle::verify` callback convention:
    /// - `None`        — cluster is being verified (read in progress)
    /// - `Some(true)`  — cluster verified successfully
    /// - `Some(false)` — cluster verification failed (hash mismatch)
    pub fn record_verify_cluster(&mut self, idx: usize, state: Option<bool>) {
        self.block_states[idx] = match state {
            None => BlockState::Verifying,
            Some(true) => BlockState::Verified,
            Some(false) => BlockState::Failed,
        };

        // Only update byte counters and speeds on completion events, not on Verifying signal
        if state.is_some() {
            self.bytes_read_total += self.block_size;

            // Update speed estimate every ~0.25 s
            let elapsed = self.last_sample_time.elapsed().as_secs_f64();
            if elapsed >= 0.25 {
                self.read_speed = (self.bytes_read_total - self.last_bytes_read) as f64 / elapsed;
                self.last_sample_time = Instant::now();
                self.last_bytes_read = self.bytes_read_total;
            }
        }
    }

    pub fn blocks_written(&self) -> usize {
        self.block_states
            .iter()
            .filter(|&&s| s == BlockState::Written)
            .count()
    }

    pub fn blocks_up_to_date(&self) -> usize {
        self.block_states
            .iter()
            .filter(|&&s| s == BlockState::UpToDate)
            .count()
    }

    pub fn blocks_processed(&self) -> usize {
        self.block_states
            .iter()
            .filter(|&&s| {
                matches!(
                    s,
                    BlockState::Written
                        | BlockState::UpToDate
                        | BlockState::Verified
                        | BlockState::Failed
                )
            })
            .count()
    }

    pub fn blocks_verified(&self) -> usize {
        self.block_states
            .iter()
            .filter(|&&s| s == BlockState::Verified)
            .count()
    }

    pub fn blocks_failed(&self) -> usize {
        self.block_states
            .iter()
            .filter(|&&s| s == BlockState::Failed)
            .count()
    }

    pub fn percentage(&self) -> f32 {
        if self.cluster_count == 0 {
            return 1.0;
        }
        self.blocks_processed() as f32 / self.cluster_count as f32
    }

    pub fn elapsed_seconds(&self) -> f64 {
        self.elapsed_final
            .unwrap_or_else(|| self.start_time.elapsed().as_secs_f64())
    }
}

pub struct AppState {
    // Image selection
    pub images: Vec<ImageHandle>,
    pub selected_image: Option<SelectedImage>,

    // Device selection
    pub devices: Vec<block_utils::Device>,
    pub selected_device: Option<block_utils::Device>,

    // Confirmation
    pub confirm_progress: f32, // 0.0 to 1.0
    pub confirm_char: char,    // Random ASCII char to press 100 times

    // Sudo re-invoke dialog
    pub show_sudo_dialog: bool,

    // Registry login
    pub registry_address: String,
    pub registry_username: String,
    pub registry_password: String,
    pub show_registry_dialog: bool,
    pub registry_login_error: Option<String>,
    pub registry_login_in_progress: bool,

    /// Authenticated registry client (shared between background threads).
    pub registry_client: Option<Arc<Mutex<Client>>>,
    /// Images available from the currently-logged-in registry.
    pub registry_images: Vec<RegistryImageEntry>,
    pub registry_list_loading: bool,
    pub registry_list_error: Option<String>,

    // Image writing progress (shared with write thread)
    pub write_progress: Option<Arc<Mutex<WriteProgress>>>,

    // Debug shell (UKI mode only)
    #[cfg(feature = "uki")]
    pub debug_shell: Option<DebugShell>,
    pub error_message: Option<String>,

    /// Non-loopback IP addresses gathered once at startup, shown in the GUI corner.
    #[cfg(feature = "uki")]
    pub ip_addresses: Vec<String>,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "uki")]
fn collect_ip_addresses() -> Vec<String> {
    use std::net::IpAddr;

    let networks = sysinfo::Networks::new_with_refreshed_list();
    let mut entries: Vec<(String, IpAddr)> = Vec::new();
    for (iface, net) in &networks {
        for ipnet in net.ip_networks() {
            if ipnet.addr.is_loopback() {
                continue;
            }
            if let IpAddr::V6(v6) = ipnet.addr {
                // Skip IPv6 link-local (fe80::/10)
                if v6.segments()[0] & 0xffc0 == 0xfe80 {
                    continue;
                }
            }
            entries.push((iface.clone(), ipnet.addr));
        }
    }
    // IPv4 first, then alphabetically by interface name for stable output.
    entries.sort_by(|a, b| {
        let a_is_v4 = matches!(a.1, IpAddr::V4(_));
        let b_is_v4 = matches!(b.1, IpAddr::V4(_));
        b_is_v4.cmp(&a_is_v4).then_with(|| a.0.cmp(&b.0))
    });
    entries
        .into_iter()
        .map(|(iface, addr)| format!("{iface}: {addr}"))
        .collect()
}

fn scan_block_devices() -> Vec<block_utils::Device> {
    let block_devices = match block_utils::get_block_devices() {
        Ok(devs) => devs,
        Err(e) => {
            warn!(error = ?e, "Failed to enumerate block devices");
            return Vec::new();
        }
    };
    debug!(count = block_devices.len(), "Found block devices");

    let devices = match block_utils::get_all_device_info(block_devices) {
        Ok(devs) => devs,
        Err(e) => {
            warn!(error = ?e, "Failed to query block device info");
            return Vec::new();
        }
    };

    let filtered: Vec<_> = devices
        .into_iter()
        .filter(|d| d.media_type != block_utils::MediaType::Loopback)
        .collect();
    debug!(
        count = filtered.len(),
        "Block devices after filtering loopbacks"
    );
    filtered
}

impl AppState {
    pub fn new() -> Self {
        Self {
            images: ImageLibrary::open()
                .find_all()
                .unwrap_or_default()
                .into_iter()
                .map(|(_host, h)| h)
                .collect(),
            selected_image: None,
            devices: scan_block_devices(),
            selected_device: None,
            confirm_progress: 0.0,
            confirm_char: {
                use rand::RngExt;
                rand::rng().sample(rand::distr::Uniform::new(b'a', b'z' + 1).unwrap()) as char
            },
            show_sudo_dialog: false,
            registry_address: String::new(),
            registry_username: String::new(),
            registry_password: String::new(),
            show_registry_dialog: false,
            registry_login_error: None,
            registry_login_in_progress: false,
            registry_client: None,
            registry_images: Vec::new(),
            registry_list_loading: false,
            registry_list_error: None,
            write_progress: None,
            #[cfg(feature = "uki")]
            debug_shell: None,
            error_message: None,
            #[cfg(feature = "uki")]
            ip_addresses: collect_ip_addresses(),
        }
    }

    /// Return the expanded byte size of the currently selected image, if one is selected.
    pub fn selected_image_size(&self) -> Option<u64> {
        match self.selected_image.as_ref()? {
            SelectedImage::Local(id) => self
                .images
                .iter()
                .find(|i| &i.id == id)
                .map(|i| i.primary_header.size),
            SelectedImage::Registry { name, tag, .. } => self
                .registry_images
                .iter()
                .find(|e| &e.name == name && &e.tag == tag)
                .map(|e| e.size),
        }
    }

    /// Load the selected image and spawn a background thread that writes it to the selected
    /// device, updating `write_progress` per cluster. Dispatches between
    /// local-library writes and streaming writes from a registry.
    pub fn start_write(&mut self) -> Result<(), String> {
        match self.selected_image.clone().ok_or("No image selected")? {
            SelectedImage::Local(id) => self.start_local_write(&id),
            SelectedImage::Registry { host: _, name, tag } => self.start_stream_write(&name, &tag),
        }
    }

    fn start_local_write(&mut self, image_id: &str) -> Result<(), String> {
        let device = self.selected_device.clone().ok_or("No device selected")?;
        let device_path = format!("/dev/{}", device.name);
        let device_size = device.capacity;

        let image_path = self
            .images
            .iter()
            .find(|i| i.id == image_id)
            .map(|i| i.path.clone())
            .ok_or("Selected image not found")?;

        let mut image = ImageHandle::open(&image_path).map_err(|e| e.to_string())?;
        image.load(None).map_err(|e| e.to_string())?;

        let cluster_count = image
            .protected_header
            .as_ref()
            .map(|h| h.cluster_count as usize)
            .unwrap_or(0);
        let block_size = image
            .protected_header
            .as_ref()
            .map(|h| h.block_size as u64)
            .unwrap_or(0);

        let progress = Arc::new(Mutex::new(WriteProgress::new(cluster_count, block_size)));
        self.write_progress = Some(progress.clone());

        std::thread::spawn(move || {
            let result = image
                .write(&device_path, can_preload(image.file_size), |idx, state| {
                    if let Ok(mut p) = progress.lock() {
                        p.record_cluster(idx, state);
                    }
                })
                .and_then(|_| {
                    let mut f = std::fs::OpenOptions::new()
                        .read(true)
                        .write(true)
                        .open(&device_path)?;
                    fixup_backup_gpt(&mut f, device_size)
                });

            if let Ok(mut p) = progress.lock() {
                p.elapsed_final = Some(p.start_time.elapsed().as_secs_f64());
                p.done = true;
                if let Err(e) = result {
                    p.error = Some(e.to_string());
                } else {
                    p.post_write_dialog = Some(PostWriteDialog::Visible);
                }
            }
        });

        Ok(())
    }

    /// Stream an image from the currently-logged-in registry directly to
    /// the selected target device. Used for UKI mode (no local staging).
    pub fn start_stream_write(&mut self, name: &str, tag: &str) -> Result<(), String> {
        let device = self.selected_device.clone().ok_or("No device selected")?;
        let device_path = format!("/dev/{}", device.name);
        let device_size = device.capacity;
        let client = self
            .registry_client
            .clone()
            .ok_or("Not logged in to a registry")?;
        let name = name.to_string();
        let tag = tag.to_string();

        // Fetch the manifest synchronously so we can build the progress
        // tracker before kicking off the streaming download.
        let (cluster_count, block_size) = {
            let client = client.lock().map_err(|_| "client poisoned")?;
            let (_p, protected, _d, digest, _start) = client
                .fetch_manifest(&name, &tag)
                .map_err(|e| e.to_string())?;
            (digest.digest_count as usize, protected.block_size as u64)
        };
        let progress = Arc::new(Mutex::new(WriteProgress::new(cluster_count, block_size)));
        self.write_progress = Some(progress.clone());

        std::thread::spawn(move || {
            let result = (|| -> anyhow::Result<()> {
                let client = client
                    .lock()
                    .map_err(|_| anyhow::anyhow!("client poisoned"))?;
                let progress_inner = progress.clone();
                client.stream_write_to_dest(
                    &name,
                    &tag,
                    std::path::Path::new(&device_path),
                    move |idx, state| {
                        if let Ok(mut p) = progress_inner.lock() {
                            p.record_cluster(idx, state);
                        }
                    },
                )?;
                // Fix up the backup GPT after writing
                let mut f = std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(&device_path)?;
                fixup_backup_gpt(&mut f, device_size)?;
                Ok(())
            })();

            if let Ok(mut p) = progress.lock() {
                p.elapsed_final = Some(p.start_time.elapsed().as_secs_f64());
                p.done = true;
                if let Err(e) = result {
                    p.error = Some(e.to_string());
                } else {
                    p.post_write_dialog = Some(PostWriteDialog::Visible);
                }
            }
        });

        Ok(())
    }

    /// Spawn a background thread that verifies the written image by hashing each block.
    /// Only supported for local-library images; registry-streamed images
    /// rely on the per-block hash check already done during write.
    pub fn start_verify(&mut self) -> Result<(), String> {
        let image_id = match self.selected_image.clone().ok_or("No image selected")? {
            SelectedImage::Local(id) => id,
            SelectedImage::Registry { .. } => {
                return Err("Verify is not supported for registry-streamed images".to_string());
            }
        };
        let device_path = self
            .selected_device
            .as_ref()
            .map(|d| format!("/dev/{}", d.name))
            .ok_or("No device selected")?;

        let image_path = self
            .images
            .iter()
            .find(|i| i.id == image_id)
            .map(|i| i.path.clone())
            .ok_or("Selected image not found")?;

        let mut image = ImageHandle::open(&image_path).map_err(|e| e.to_string())?;
        image.load(None).map_err(|e| e.to_string())?;

        let progress = self
            .write_progress
            .clone()
            .ok_or("No write progress to verify")?;

        // Reset block states for verification pass
        if let Ok(mut p) = progress.lock() {
            for s in p.block_states.iter_mut() {
                *s = BlockState::Pending;
            }
            p.done = false;
            p.verifying = true;
            p.post_write_dialog = None;
            p.elapsed_final = None;
            p.start_time = Instant::now();
        }

        std::thread::spawn(move || {
            let result = image.verify(&device_path, |idx, state| {
                if let Ok(mut p) = progress.lock() {
                    p.record_verify_cluster(idx, state);
                }
            });

            if let Ok(mut p) = progress.lock() {
                p.elapsed_final = Some(p.start_time.elapsed().as_secs_f64());
                p.done = true;
                p.verifying = false;
                if let Err(e) = result {
                    p.error = Some(e.to_string());
                }
                p.post_write_dialog = Some(PostWriteDialog::Visible);
            }
        });

        Ok(())
    }

    /// Spawn a background thread that verifies the device against the image (without writing first).
    pub fn start_verify_only(&mut self) -> Result<(), String> {
        let image_id = match self.selected_image.clone().ok_or("No image selected")? {
            SelectedImage::Local(id) => id,
            SelectedImage::Registry { .. } => {
                return Err("Verify-only is not supported for registry images yet".to_string());
            }
        };
        let device_path = self
            .selected_device
            .as_ref()
            .map(|d| format!("/dev/{}", d.name))
            .ok_or("No device selected")?;

        let image_path = self
            .images
            .iter()
            .find(|i| i.id == image_id)
            .map(|i| i.path.clone())
            .ok_or("Selected image not found")?;

        let mut image = ImageHandle::open(&image_path).map_err(|e| e.to_string())?;
        image.load(None).map_err(|e| e.to_string())?;

        let cluster_count = image
            .protected_header
            .as_ref()
            .map(|h| h.cluster_count as usize)
            .unwrap_or(0);
        let block_size = image
            .protected_header
            .as_ref()
            .map(|h| h.block_size as u64)
            .unwrap_or(0);

        let progress = Arc::new(Mutex::new(WriteProgress::new(cluster_count, block_size)));
        if let Ok(mut p) = progress.lock() {
            p.verifying = true;
            p.verify_only = true;
        }
        self.write_progress = Some(progress.clone());

        std::thread::spawn(move || {
            let result = image.verify(&device_path, |idx, state| {
                if let Ok(mut p) = progress.lock() {
                    p.record_verify_cluster(idx, state);
                }
            });

            if let Ok(mut p) = progress.lock() {
                p.elapsed_final = Some(p.start_time.elapsed().as_secs_f64());
                p.done = true;
                p.verifying = false;
                if let Err(e) = result {
                    p.error = Some(e.to_string());
                }
                p.post_write_dialog = Some(PostWriteDialog::Visible);
            }
        });

        Ok(())
    }
}
