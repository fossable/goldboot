use crate::cache::MediaCache;
use crate::qemu::QemuArgs;
use serde::{Deserialize, Serialize};
use std::error::Error;
use validator::Validate;
use goldboot_core::*;

#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct SteamDeckTemplate {
    pub recovery_url: String,

    pub recovery_checksum: String,
}

impl Default for SteamDeckTemplate {
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

impl Template for SteamDeckTemplate {
    fn build(&self, context: &BuildContext) -> Result<(), Box<dyn Error>> {
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
            // Initial wait
            wait!(20),
            // Wait for login
            wait_screen_rect!("ba99ede257ef4ee2056a328eb3feffa65e821e0d", 0, 0, 1024, 700),
            // Open terminal
            leftSuper!(), enter!("terminal"),
            // Disable Zenity prompt
            enter!("sed -i '/zenity/d' ./tools/repair_device.sh"),
            // Poweroff instead of reboot on completion
            enter!("sed -i 's/systemctl reboot/systemctl poweroff/' ./tools/repair_device.sh"),
            // Begin reimage
            enter!("./tools/repair_reimage.sh"),
        ])?;

        // Wait for shutdown
        qemu.shutdown_wait()?;

        Ok(())
    }
}
