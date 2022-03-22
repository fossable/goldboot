use crate::cache::MediaCache;
use crate::qemu::QemuArgs;
use crate::{
    config::Config,
    profile::Profile,
    vnc::bootcmds::{enter, wait},
};
use serde::{Deserialize, Serialize};
use std::error::Error;
use validator::Validate;

#[derive(Clone, Serialize, Deserialize, Validate)]
pub struct SteamDeckProfile {
    pub recovery_url: String,

    pub recovery_checksum: String,
}

impl Default for SteamDeckProfile {
    fn default() -> Self {
        Self {
            recovery_url: String::from(
                "https://steamdeck-images.steamos.cloud/recovery/steamdeck-recovery-1.img.bz2",
            ),
            recovery_checksum: String::from("sha256:5086bcc4fe0fb230dff7265ff6a387dd00045e3d9ae6312de72003e1e82d4526"),
        }
    }
}

impl Profile for SteamDeckProfile {
    fn build(&self, config: &Config, image_path: &str) -> Result<(), Box<dyn Error>> {
        let mut qemuargs = QemuArgs::new(&config);

        qemuargs.add_drive(image_path, "virtio");
        qemuargs.add_cdrom(MediaCache::get_bzip2(
            self.recovery_url.clone(),
            &self.recovery_checksum,
        )?);

        // Start VM
        let mut qemu = qemuargs.start_process()?;

        // Send boot command
        qemu.vnc.boot_command(vec![
            wait!(20),  // Wait for boot
            enter!(),   // Begin auto install
            wait!(600), // Wait for install
        ])?;

        Ok(())
    }
}
