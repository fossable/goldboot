use crate::cache::MediaCache;
use crate::config::Provisioner;
use crate::qemu::QemuArgs;
use crate::{
    config::Config,
    profile::Profile,
    vnc::bootcmds::{enter, wait},
};
use serde::{Deserialize, Serialize};
use std::error::Error;
use validator::Validate;

#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct SteamOsProfile {
    pub iso_url: String,

    pub iso_checksum: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub provisioners: Option<Vec<Provisioner>>,
}

impl Default for SteamOsProfile {
    fn default() -> Self {
        Self {
            iso_url: String::from(
                "https://repo.steampowered.com/download/brewmaster/2.195/SteamOSDVD.iso",
            ),
            iso_checksum: String::from("none"),
            provisioners: None,
        }
    }
}

impl Profile for SteamOsProfile {
    fn build(
        &self,
        config: &Config,
        image_path: &str,
    ) -> Result<(), Box<dyn Error>> {
        let mut qemuargs = QemuArgs::new(&config);

        qemuargs.drive.push(format!(
            "file={image_path},if=virtio,cache=writeback,discard=ignore,format=qcow2"
        ));
        qemuargs.drive.push(format!(
            "file={},media=cdrom",
            MediaCache::get(self.iso_url.clone(), &self.iso_checksum)?
        ));

        // Start VM
        let mut qemu = qemuargs.start_process()?;

        // Send boot command
        qemu.vnc.boot_command(vec![
            wait!(20), // Wait for boot
            enter!(),
            wait!(600),
        ])?;

        // Wait for SSH
        let ssh = qemu.ssh()?;

        // Run provisioners
        for provisioner in &self.provisioners {
            // TODO
        }

        // Shutdown
        qemu.shutdown("poweroff")?;
        Ok(())
    }
}
