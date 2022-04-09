use crate::cache::MediaCache;
use crate::config::Provisioner;
use crate::qemu::QemuArgs;
use crate::{
    config::Config,
    profile::Profile,
    vnc::bootcmds::{enter, wait_screen},
};
use serde::{Deserialize, Serialize};
use std::error::Error;
use validator::Validate;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum SteamOsVersion {
    Brewmaster2_195,
}

#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct SteamOsProfile {
    pub version: SteamOsVersion,

    pub iso_url: String,

    pub iso_checksum: String,

    pub root_password: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub provisioners: Option<Vec<Provisioner>>,
}

impl Default for SteamOsProfile {
    fn default() -> Self {
        Self {
            iso_url: String::from(
                "https://repo.steampowered.com/download/brewmaster/2.195/SteamOSDVD.iso",
            ),
            iso_checksum: String::from("sha512:0ce55048d2c5e8a695f309abe22303dded003c93386ad28c6daafc977b3d5b403ed94d7c38917c8c837a2b1fe560184cf3cc12b9f2c4069fd70ed0deab47eb7c"),
            root_password: String::from("root"),
            provisioners: None,
            version: SteamOsVersion::Brewmaster2_195,
        }
    }
}

impl Profile for SteamOsProfile {
    fn build(&self, config: &Config, image_path: &str) -> Result<(), Box<dyn Error>> {
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
        #[rustfmt::skip]
        qemu.vnc.boot_command(vec![
            // Wait for bootloader
            wait_screen!("28fe084e08242584908114a5d21960fdf072adf9"),
            // Start automated install
            enter!(),
            // Wait for completion
            wait_screen!(""),
        ])?;

        // Wait for SSH
        let ssh = qemu.ssh_wait(config.ssh_port.unwrap(), "root", &self.root_password)?;

        // Run provisioners
        for provisioner in &self.provisioners {
            // TODO
        }

        // Shutdown
        ssh.shutdown("poweroff")?;
        qemu.shutdown_wait()?;
        Ok(())
    }
}
