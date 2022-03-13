use crate::{
    config::Config,
    packer::bootcmds::{enter, input, leftSuper, spacebar, tab, wait},
    packer::{PackerTemplate, QemuBuilder},
    profile::Profile,
};
use serde::{Deserialize, Serialize};
use std::{error::Error, path::Path};
use validator::Validate;

#[derive(Clone, Serialize, Deserialize, Validate, Default)]
pub struct SteamOsProfile {
    pub version: String,

    #[serde(default = "default_iso_url")]
    iso_url: String,

    #[serde(default = "default_iso_checksum")]
    iso_checksum: String,
}

fn default_iso_url() -> String {
    String::from("https://repo.steampowered.com/download/brewmaster/2.195/SteamOSDVD.iso")
}

fn default_iso_checksum() -> String {
    String::from("none")
}

impl Profile for SteamOsProfile {
    fn generate_template(&self, context: &Path) -> Result<PackerTemplate, Box<dyn Error>> {
        let mut template = PackerTemplate::default();

        let mut builder = QemuBuilder::new();
        builder.boot_command = vec![
            enter!(),   // Begin auto install
            wait!(600), // Wait for install
        ];

        builder.boot_wait = String::from("20s");
        builder.communicator = String::from("ssh");
        builder.shutdown_command = String::from("poweroff");
        builder.ssh_password = Some(String::from("root"));
        builder.ssh_wait_timeout = Some(String::from("5m"));

        template.builders.push(builder);

        Ok(template)
    }
}
