use crate::cache::MediaCache;
use crate::qemu::QemuArgs;
use crate::templates::*;
use serde::{Deserialize, Serialize};
use std::error::Error;
use validator::Validate;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum SteamOsVersion {
    Brewmaster2_195,
}

#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct SteamOsTemplate {
    pub version: SteamOsVersion,

    pub iso_url: String,

    pub iso_checksum: String,

    pub root_password: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub provisioners: Option<Vec<Provisioner>>,
}

impl Default for SteamOsTemplate {
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

impl Template for SteamOsTemplate {
    fn build(&self, context: &BuildContext) -> Result<(), Box<dyn Error>> {
        let mut qemuargs = QemuArgs::new(&context);

        qemuargs.drive.push(format!(
            "file={},if=virtio,cache=writeback,discard=ignore,format=qcow2",
            context.image_path
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
        let ssh = qemu.ssh_wait(context.ssh_port, "root", &self.root_password)?;

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
