use crate::cache::MediaCache;
use crate::qemu::QemuArgs;
use crate::{
    config::Config,
    image_cache_lookup,
    packer::bootcmds::{enter, input, leftSuper, spacebar, tab, wait},
    packer::{PackerTemplate, QemuBuilder},
    profile::Profile,
};
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

        Ok(template)
    }
}

impl SteamDeckProfile {
    fn build(&self, config: Config, image: &Path) -> Result<(), Box<dyn Error>> {
        let mut qemuargs = QemuArgs::new(&config);

        qemuargs.add_drive(image.to_string_lossy().to_string());
        qemuargs.add_cdrom(MediaCache::get_bzip2(self.recovery_url.clone(), self.recovery_checksum.clone())?);

        // Start VM
        let qemu = qemuargs.start_process()?;

        // Send boot command
        qemu.vnc.boot_command(vec![
            wait!(20),  // Wait for boot
            enter!(),   // Begin auto install
            wait!(600), // Wait for install
        ]);

        Ok(())
    }
}