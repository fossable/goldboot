use crate::{
    config::Config,
    image_cache_lookup,
    packer::bootcmds::{enter, input, leftSuper, spacebar, tab, wait},
    packer::{PackerTemplate, QemuBuilder},
    profile::Profile,
};
use bzip2_rs::DecoderReader;
use serde::{Deserialize, Serialize};
use std::{error::Error, fs::File, io, path::Path};
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
            recovery_checksum: String::from("none"),
        }
    }
}

impl Profile for SteamDeckProfile {
    fn generate_template(&self, context: &Path) -> Result<PackerTemplate, Box<dyn Error>> {
        let mut template = PackerTemplate::default();

        // Check the cache
        let recovery_file = image_cache_lookup(&self.recovery_url);
        if !recovery_file.is_file() {
            let rs = reqwest::blocking::get(&self.recovery_url)?;
            if rs.status().is_success() {
                let mut reader = DecoderReader::new(rs);
                io::copy(&mut reader, &mut File::open(recovery_file)?)?;
            }
        }

        let mut builder = QemuBuilder::new();
        builder.boot_command = vec![
            enter!(),   // Begin auto install
            wait!(600), // Wait for install
        ];

        builder.boot_wait = String::from("20s");
        builder.communicator = String::from("none");

        template.builders.push(builder);

        Ok(template)
    }
}
