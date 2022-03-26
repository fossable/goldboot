use crate::cache::MediaCache;
use crate::config::Config;
use crate::qemu::QemuArgs;
use crate::{
    config::{Partition, Provisioner},
    profile::Profile,
    vnc::bootcmds::{enter, wait},
};
use serde::{Deserialize, Serialize};
use std::error::Error;
use validator::Validate;

const DEFAULT_MIRROR: &str = "https://dl-cdn.alpinelinux.org/alpine";

#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct AlpineProfile {
    pub root_password: String,

    /// The installation media URL
    pub iso_url: String,

    /// A hash of the installation media
    pub iso_checksum: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub partitions: Option<Vec<Partition>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub provisioners: Option<Vec<Provisioner>>,
}

impl Default for AlpineProfile {
    fn default() -> Self {
        Self {
            root_password: String::from("root"),
            iso_url: String::from("https://dl-cdn.alpinelinux.org/alpine/v3.15/releases/x86_64/alpine-standard-3.15.0-x86_64.iso"),
            iso_checksum: String::from("none"),
            partitions: None,
            provisioners: None,
        }
    }
}

impl Profile for AlpineProfile {
    fn build(
        &self,
        config: &Config,
        image_path: &str,
        record: bool,
        debug: bool,
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
        let mut qemu = qemuargs.start_process(record, debug)?;

        // Send boot command
        qemu.vnc.boot_command(vec![
            wait!(60),                                    // Wait for boot
            enter!("root"),                               // Login as root
            enter!("KEYMAPOPTS='us us' setup-alpine -q"), // Start quick install
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
