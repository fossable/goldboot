use crate::{
    config::{Partition, Provisioner},
    packer::bootcmds::enter,
    packer::{PackerProvisioner, PackerTemplate, QemuBuilder, ShellPackerProvisioner},
    profile::Profile,
    scale_wait_time,
};
use serde::{Deserialize, Serialize};
use simple_error::bail;
use std::{error::Error, io::BufRead, io::BufReader, path::Path};
use validator::Validate;

const DEFAULT_MIRROR: &str = "https://dl-cdn.alpinelinux.org/alpine";

#[derive(Clone, Serialize, Deserialize, Validate)]
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
    fn generate_template(&self, context: &Path) -> Result<PackerTemplate, Box<dyn Error>> {
        let mut template = PackerTemplate::default();

        let mut builder = QemuBuilder::new();
        builder.boot_command = vec![
            enter!("root"),
            enter!("KEYMAPOPTS='us us' setup-alpine -q"),
        ];
        builder.boot_wait = scale_wait_time(700000);
        builder.communicator = String::from("ssh");
        builder.shutdown_command = String::from("poweroff");
        builder.ssh_password = Some(String::from("root"));
        builder.ssh_username = Some(String::from("root"));
        builder.ssh_wait_timeout = Some(String::from("1m"));
        builder.iso_url = self.iso_url.clone();
        builder.iso_checksum = self.iso_checksum.clone();
        builder.qemuargs = vec![vec![
            String::from("-global"),
            String::from("driver=cfi.pflash01,property=secure,value=on"),
        ]];

        template.builders.push(builder);

        Ok(template)
    }
}