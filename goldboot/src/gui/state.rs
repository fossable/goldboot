use crate::{can_preload, gpt::fixup_backup_gpt};
use goldboot_image::ImageHandle;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[cfg(feature = "uki")]
pub struct DebugShell {
    pub terminal_backend: egui_term::TerminalBackend,
    pub pty_event_receiver: std::sync::mpsc::Receiver<(u64, egui_term::PtyEvent)>,
}


#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BlockState {
    Pending,   // Not yet processed
    Writing,   // Currently being written
    UpToDate,  // Block was already correct, no write needed
    Written,   // Block was dirty and has been written
    Verifying, // Currently being read and hashed
    Verified,  // Hash matched
    Failed,    // Hash did not match (corruption)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PostWriteDialog {
    Visible, // Waiting for user choice
    Hidden,  // Dialog dismissed (verifying or done)
}

pub struct WriteProgress {
    pub cluster_count: usize,                       // Total number of clusters
    pub block_size: u64,                            // Bytes per cluster
    pub block_states: Vec<BlockState>,              // Per-cluster state
    pub done: bool,                                 // Write (or verify) thread has finished
    pub verifying: bool,                            // Verification pass is running
    pub post_write_dialog: Option<PostWriteDialog>, // Dialog state after write completes
    pub error: Option<String>,                      // Set if write or verify failed
    pub start_time: Instant,

    // Speed tracking: bytes read/written since last speed sample
    bytes_read_total: u64,
    bytes_written_total: u64,
    last_sample_time: Instant,
    last_bytes_read: u64,
    last_bytes_written: u64,
    pub read_speed: f64,  // bytes/sec, updated each cluster
    pub write_speed: f64, // bytes/sec, updated each cluster
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
            post_write_dialog: None,
            error: None,
            start_time: now,
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
        self.start_time.elapsed().as_secs_f64()
    }
}

pub struct AppState {
    // Image selection
    pub images: Vec<ImageHandle>,
    pub selected_image: Option<String>,

    // Device selection
    pub devices: Vec<block_utils::Device>,
    pub selected_device: Option<block_utils::Device>,

    // Confirmation
    pub confirm_progress: f32, // 0.0 to 1.0
    pub confirm_char: char,    // Random ASCII char to press 100 times

    // Registry login
    pub registry_address: String,
    pub registry_password: String,
    pub show_registry_dialog: bool,
    pub registry_login_error: Option<String>,

    // Image writing progress (shared with write thread)
    pub write_progress: Option<Arc<Mutex<WriteProgress>>>,

    // Debug shell (UKI mode only)
    #[cfg(feature = "uki")]
    pub debug_shell: Option<DebugShell>,
    pub error_message: Option<String>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            images: crate::library::ImageLibrary::find_all().unwrap_or_default(),
            selected_image: None,
            devices: {
                let block_devices = block_utils::get_block_devices().unwrap_or_default();
                let devices = block_utils::get_all_device_info(block_devices).unwrap_or_default();
                devices
                    .into_iter()
                    .filter(|d| d.media_type != block_utils::MediaType::Loopback)
                    .collect()
            },
            selected_device: None,
            confirm_progress: 0.0,
            confirm_char: {
                use rand::Rng;
                rand::rng().sample(rand::distr::Uniform::new(b'a', b'z' + 1).unwrap()) as char
            },
            registry_address: String::new(),
            registry_password: String::new(),
            show_registry_dialog: false,
            registry_login_error: None,
            write_progress: None,
            #[cfg(feature = "uki")]
            debug_shell: None,
            error_message: None,
        }
    }

    /// Load the selected image and spawn a background thread that writes it to the selected
    /// device, updating `write_progress` per cluster.
    pub fn start_write(&mut self) -> Result<(), String> {
        let image_id = self.selected_image.clone().ok_or("No image selected")?;
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
    pub fn start_verify(&mut self) -> Result<(), String> {
        let image_id = self.selected_image.clone().ok_or("No image selected")?;
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
            p.start_time = Instant::now();
        }

        std::thread::spawn(move || {
            let result = image.verify(&device_path, |idx, state| {
                if let Ok(mut p) = progress.lock() {
                    p.block_states[idx] = match state {
                        None => BlockState::Verifying,
                        Some(true) => BlockState::Verified,
                        Some(false) => BlockState::Failed,
                    };
                }
            });

            if let Ok(mut p) = progress.lock() {
                p.done = true;
                p.verifying = false;
                if let Err(e) = result {
                    p.error = Some(e.to_string());
                }
            }
        });

        Ok(())
    }
}
