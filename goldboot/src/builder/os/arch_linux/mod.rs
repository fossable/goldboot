use super::BuildImage;
use crate::builder::http::HttpServer;
use crate::builder::options::arch::Arch;
use crate::builder::options::hostname::Hostname;
use crate::builder::options::iso::Iso;
use crate::builder::options::locale::Locale;
use crate::builder::options::ntp::Ntp;
use crate::builder::options::packages::Packages;
use crate::builder::options::size::Size;
use crate::builder::options::timezone::Timezone;
use crate::builder::options::unix_account::RootPassword;
use crate::builder::options::unix_users::UnixUsers;
use crate::builder::os::arch_linux::archinstall::ArchinstallConfig;
use crate::builder::os::arch_linux::archinstall::ArchinstallCredentials;
use crate::builder::qemu::{OsCategory, QemuBuilder};
use crate::cli::prompt::Prompt;
use crate::wait;
use crate::{builder::Builder, wait_text};
use anyhow::Result;
use anyhow::bail;
use goldboot_image::ImageArch;
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;
use std::io::{BufRead, BufReader};
use tracing::{debug, info};
use validator::Validate;

mod archinstall;

/// Recursively merge `overlay` into `base`. Object fields in `overlay` overwrite
/// matching fields in `base`; all other values replace `base` entirely.
fn json_merge(base: &mut serde_json::Value, overlay: serde_json::Value) {
    match (base, overlay) {
        (serde_json::Value::Object(base_map), serde_json::Value::Object(overlay_map)) => {
            for (key, val) in overlay_map {
                json_merge(base_map.entry(key).or_insert(serde_json::Value::Null), val);
            }
        }
        (base, overlay) => *base = overlay,
    }
}

fn default_arch() -> Arch {
    Arch(ImageArch::Amd64)
}

fn default_iso() -> Iso {
    Iso {
        url: "http://mirrors.edge.kernel.org/archlinux/iso/latest/archlinux-2026.03.01-x86_64.iso"
            .parse()
            .unwrap(),
        checksum: None,
    }
}

/// Arch Linux is an independently developed x86-64 general-purpose Linux distribution
/// that strives to provide the latest stable versions of most software by following
/// a rolling-release model.
///
/// Upstream: https://archlinux.org
/// Maintainer: cilki
#[goldboot_macros::Os(architectures(Amd64))]
#[derive(Clone, Serialize, Deserialize, Validate, Debug, SmartDefault, goldboot_macros::Prompt)]
pub struct ArchLinux {
    #[default(Arch(ImageArch::Amd64))]
    #[serde(default = "default_arch")]
    pub arch: Arch,
    pub size: Size,
    #[serde(default)]
    pub hostname: Hostname,
    pub mirrorlist: Option<ArchLinuxMirrorlist>,
    pub packages: Option<Packages>,
    #[serde(default)]
    pub root_password: RootPassword,

    /// Additional user accounts to create
    pub users: Option<UnixUsers>,

    /// Kernel packages to install (e.g. "linux", "linux-lts", "linux-zen")
    #[serde(default)]
    pub kernels: ArchLinuxKernels,

    /// System timezone (e.g. "UTC", "America/New_York")
    #[serde(default)]
    pub timezone: Timezone,

    /// Locale configuration
    #[serde(default)]
    pub locale: Locale,

    /// Audio server to install
    pub audio: Option<ArchLinuxAudio>,

    /// Desktop environment / window manager profile
    pub profile: Option<ArchLinuxProfile>,

    // TODO grub themes
    /// Bootloader configuration
    #[serde(default)]
    pub bootloader: ArchLinuxBootloader,

    /// Swap configuration
    #[serde(default)]
    pub swap: ArchLinuxSwap,

    /// Full-disk encryption passphrase (LUKS)
    pub encryption_password: Option<ArchLinuxEncryptionPassword>,

    /// Enable NTP time synchronization
    #[serde(default)]
    pub ntp: Ntp,

    /// Number of parallel pacman downloads (0 = default)
    #[serde(default)]
    pub parallel_downloads: ArchLinuxParallelDownloads,

    /// Path to an existing archinstall config JSON to merge with the generated config.
    /// Fields in the user-provided config take precedence over goldboot's defaults.
    pub archinstall_config: Option<ArchLinuxConfigPath>,

    #[serde(default = "default_iso")]
    #[default(_code = "default_iso()")]
    pub iso: Iso,
}

impl BuildImage for ArchLinux {
    fn build(&self, worker: &Builder) -> Result<()> {
        let mut qemu = QemuBuilder::new(&worker, OsCategory::Linux)
            .with_iso(&self.iso)?
            .prepare_ssh()?
            .start()?;

        // Generate an archinstall config, optionally merging a user-supplied one
        let archinstall_config = if let Some(path) = &self.archinstall_config {
            let user_bytes = std::fs::read(&path.0)?;
            let mut base: serde_json::Value =
                serde_json::from_str(&serde_json::to_string(&ArchinstallConfig::from(self))?)?;
            let overlay: serde_json::Value = serde_json::from_slice(&user_bytes)?;
            json_merge(&mut base, overlay);
            serde_json::from_value::<ArchinstallConfig>(base)?
        } else {
            ArchinstallConfig::from(self)
        };
        debug!(config = %serde_json::to_string_pretty(&archinstall_config)?, "Archinstall config");

        let archinstall_creds = ArchinstallCredentials::from(self);

        // Start HTTP
        let http = HttpServer::new()?
            .file("config.json", serde_json::to_vec(&archinstall_config)?)?
            .file("creds.json", serde_json::to_vec(&archinstall_creds)?)?
            .serve();

        // Send boot command
        if !worker.has_checkpoint("boot") {
            #[rustfmt::skip]
    		qemu.vnc.run(vec![
    			// Initial wait
    			wait!(30),
    			// Wait for login
    			wait_text!("root.archiso"),
    		])?;

            qemu.install_ssh()?;
            qemu.checkpoint("boot")?;
        }

        // Wait for SSH
        let mut ssh = qemu.ssh("root")?;

        if !worker.has_checkpoint("install") {
            // Run install script
            info!("Running base installation");
            match ssh.upload_exec(
                include_bytes!("bootstrap.sh"),
                vec![
                    ("GB_HTTP_HOST", &http.address),
                    ("GB_HTTP_PORT", &format!("{}", &http.port)),
                ],
            ) {
                Ok(0) => debug!("Installation completed successfully"),
                _ => bail!("Installation failed"),
            }

            qemu.checkpoint("install")?;
        }

        // Shutdown
        ssh.shutdown("poweroff")?;
        qemu.shutdown_wait()?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Supporting types
// ---------------------------------------------------------------------------

/// This provisioner configures the Archlinux mirror list.
#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct ArchLinuxMirrorlist {
    pub mirrors: Vec<String>,
}

//https://archlinux.org/mirrorlist/?country=US&protocol=http&protocol=https&ip_version=4

impl Default for ArchLinuxMirrorlist {
    fn default() -> Self {
        Self {
            mirrors: vec![
                String::from("https://geo.mirror.pkgbuild.com/"),
                String::from("https://mirror.rackspace.com/archlinux/"),
                String::from("https://mirrors.edge.kernel.org/archlinux/"),
            ],
        }
    }
}

impl Prompt for ArchLinuxMirrorlist {
    fn prompt(&mut self, _: &Builder) -> Result<()> {
        Ok(())
    }
}

impl ArchLinuxMirrorlist {
    pub fn format_mirrorlist(&self) -> String {
        self.mirrors
            .iter()
            .map(|s| format!("Server = {}", s))
            .collect::<Vec<String>>()
            .join("\n")
    }
}

/// Fetch the latest installation ISO
#[allow(dead_code)]
fn fetch_latest_iso() -> Result<Iso> {
    let rs = reqwest::blocking::get(format!(
        "http://mirrors.edge.kernel.org/archlinux/iso/latest/sha256sums.txt"
    ))?;
    if rs.status().is_success() {
        for line in BufReader::new(rs).lines().filter_map(|result| result.ok()) {
            if line.ends_with(".iso") {
                let split: Vec<&str> = line.split_whitespace().collect();
                if let [hash, filename] = split[..] {
                    return Ok(Iso {
                        url: format!(
                            "http://mirrors.edge.kernel.org/archlinux/iso/latest/{filename}"
                        )
                        .parse()
                        .unwrap(),
                        checksum: Some(format!("sha256:{hash}")),
                    });
                }
            }
        }
    }
    bail!("Failed to request latest ISO");
}

/// Kernel packages to install (e.g. "linux", "linux-lts", "linux-zen").
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ArchLinuxKernels(pub Vec<String>);

impl Default for ArchLinuxKernels {
    fn default() -> Self {
        Self(vec!["linux".to_string()])
    }
}

impl Prompt for ArchLinuxKernels {
    fn prompt(&mut self, _: &Builder) -> Result<()> {
        Ok(())
    }
}

/// Available audio servers.
#[derive(Clone, Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub enum ArchLinuxAudio {
    #[default]
    Pipewire,
    Pulseaudio,
}

impl Prompt for ArchLinuxAudio {
    fn prompt(&mut self, _: &Builder) -> Result<()> {
        Ok(())
    }
}

/// Desktop environment / window manager profile.
#[derive(Clone, Serialize, Deserialize, Validate, Debug, SmartDefault)]
pub struct ArchLinuxProfile {
    /// Profile name (e.g. "gnome", "kde", "sway", "i3", "xfce", "minimal", "server")
    pub name: String,
    /// Additional sub-profiles or variant details
    #[serde(default)]
    pub details: Vec<String>,
    /// Graphics driver to install (e.g. "All open-source", "Nvidia", "AMD / ATI")
    pub gfx_driver: Option<String>,
    /// Display manager / greeter (e.g. "sddm", "gdm", "lightdm")
    pub greeter: Option<String>,
}

impl Prompt for ArchLinuxProfile {
    fn prompt(&mut self, _: &Builder) -> Result<()> {
        Ok(())
    }
}

/// Bootloader selection.
#[derive(Clone, Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub enum ArchLinuxBootloaderKind {
    #[default]
    Grub,
    Systemd,
}

impl Prompt for ArchLinuxBootloaderKind {
    fn prompt(&mut self, _: &Builder) -> Result<()> {
        Ok(())
    }
}

#[derive(Clone, Serialize, Deserialize, Validate, Debug, SmartDefault)]
pub struct ArchLinuxBootloader {
    #[default(ArchLinuxBootloaderKind::Grub)]
    pub kind: ArchLinuxBootloaderKind,
    /// Install as a unified kernel image (UKI)
    #[default(false)]
    pub uki: bool,
    /// Install to removable media path (EFI/BOOT/BOOTX64.EFI).
    /// Defaults to true so images boot on hardware with no pre-existing NVRAM entries.
    #[default(true)]
    pub removable: bool,
}

impl Prompt for ArchLinuxBootloader {
    fn prompt(&mut self, _: &Builder) -> Result<()> {
        Ok(())
    }
}

/// Swap configuration.
#[derive(Clone, Serialize, Deserialize, Validate, Debug, SmartDefault)]
pub struct ArchLinuxSwap {
    #[default(false)]
    pub enabled: bool,
    /// Compression algorithm for zram swap (e.g. "zstd", "lz4")
    #[default("zstd".to_string())]
    pub algorithm: String,
}

impl Prompt for ArchLinuxSwap {
    fn prompt(&mut self, _: &Builder) -> Result<()> {
        Ok(())
    }
}

/// Full-disk encryption passphrase (LUKS).
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ArchLinuxEncryptionPassword(pub String);

impl Prompt for ArchLinuxEncryptionPassword {
    fn prompt(&mut self, _: &Builder) -> Result<()> {
        Ok(())
    }
}

/// Number of parallel pacman downloads (0 = pacman default).
#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct ArchLinuxParallelDownloads(pub u32);

impl Prompt for ArchLinuxParallelDownloads {
    fn prompt(&mut self, _: &Builder) -> Result<()> {
        Ok(())
    }
}

/// Path to an existing archinstall config JSON to merge with the generated config.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ArchLinuxConfigPath(pub std::path::PathBuf);

impl Prompt for ArchLinuxConfigPath {
    fn prompt(&mut self, _: &Builder) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fetch_latest_iso() -> Result<()> {
        fetch_latest_iso()?;
        Ok(())
    }
}
