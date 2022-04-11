use goldboot_core::cache::MediaCache;
use goldboot_core::qemu::QemuArgs;
use goldboot_core::*;
use serde::{Deserialize, Serialize};
use std::error::Error;
use validator::Validate;

const DEFAULT_MIRROR: &str = "https://dl-cdn.alpinelinux.org/alpine";

#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct AlpineTemplate {
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

impl Default for AlpineTemplate {
    fn default() -> Self {
        Self {
            root_password: String::from("root"),
            iso_url: format!("{DEFAULT_MIRROR}/v3.15/releases/x86_64/alpine-standard-3.15.0-x86_64.iso"),
            iso_checksum: String::from("none"),
            partitions: None,
            provisioners: None,
        }
    }
}

impl Template for AlpineTemplate {
    fn build(&self, context: &BuildContext) -> Result<(), Box<dyn Error>> {
        let mut qemuargs = QemuArgs::new(&context);

        qemuargs.drive.push(format!(
            "file={},if=virtio,cache=writeback,discard=ignore,format=qcow2", context.image_path
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
            // Initial wait
            wait!(60),
            // Root login
            enter!("root"),
            // Start quick install
            enter!("KEYMAPOPTS='us us' setup-alpine -q"),
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
