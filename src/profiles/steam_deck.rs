use crate::cache::MediaCache;
use crate::qemu::QemuArgs;
use crate::{
    config::Config,
    profile::Profile,
    vnc::bootcmds::{enter, leftSuper, wait, wait_screen_rect},
};
use serde::{Deserialize, Serialize};
use std::error::Error;
use validator::Validate;

#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
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
            recovery_checksum: String::from(
                "sha256:5086bcc4fe0fb230dff7265ff6a387dd00045e3d9ae6312de72003e1e82d4526",
            ),
        }
    }
}

impl Profile for SteamDeckProfile {
    fn build(&self, config: &Config, image_path: &str) -> Result<(), Box<dyn Error>> {
        let mut qemuargs = QemuArgs::new(&config);

        qemuargs.drive.push(format!(
            "file={},format=raw",
            MediaCache::get_bzip2(self.recovery_url.clone(), &self.recovery_checksum,)?
        ));
        qemuargs.drive.push(format!(
            "file={image_path},if=none,cache=writeback,discard=ignore,format=qcow2,id=nvme"
        ));

        // Make the storage looks like an nvme drive
        qemuargs
            .device
            .push(String::from("nvme,serial=cafebabe,drive=nvme"));

        // Start VM
        let mut qemu = qemuargs.start_process()?;

        // Send boot command
        #[rustfmt::skip]
        qemu.vnc.boot_command(vec![
            wait!(20),  // Boot wait
            wait_screen_rect!("27a67174b8bf68ec46866b2210239db32a25d2de", 0, 0, 500, 500), // Wait for login
            leftSuper!(), enter!("terminal"),   // Open terminal
            enter!("./tools/repair_reimage.sh"), // Begin reimage
            wait!(600), // Wait for install
        ])?;

        Ok(())
    }
}
