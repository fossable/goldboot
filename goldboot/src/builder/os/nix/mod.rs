use anyhow::Result;
use goldboot_image::ImageArch;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf};
use validator::Validate;

use crate::{
    builder::{
        Builder,
        options::iso::Iso,
        qemu::{OsCategory, QemuBuilder},
    },
    cli::prompt::Prompt,
    enter, wait, wait_screen_rect,
};

use super::BuildImage;

/// NixOS is a free and open source Linux distribution based on the Nix package
/// manager. NixOS uses an immutable design and an atomic update model. Its use
/// of a declarative configuration system allows reproducibility and
/// portability.
///
/// Upstream: https://www.nixos.org
/// Maintainer: cilki
#[derive(Clone, Serialize, Deserialize, Validate, Debug, goldboot_macros::Prompt)]
pub struct Nix {
    /// Path to /etc/nixos/configuration.nix
    pub configuration: ConfigurationPath,

    /// Path to /etc/nixos/hardware-configuration.nix
    pub hardware_configuration: Option<ConfigurationPath>,

    pub iso: Iso,
}

impl Default for Nix {
    fn default() -> Self {
        Self {
            configuration: ConfigurationPath(PathBuf::from("configuration.nix")),
            hardware_configuration: None,
            iso: Iso {
                url: "https://example.com".parse().unwrap(),
                checksum: None,
            },
        }
    }
}

impl BuildImage for Nix {
    fn build(&self, worker: &Builder) -> Result<()> {
        let mut qemu = QemuBuilder::new(&worker, OsCategory::Linux)
            .with_iso(&self.iso)?
            // Add Nix config
            .drive_files(HashMap::from([(
                "configuration.nix".to_string(),
                self.configuration.load()?,
            )]))?
            .start()?;

        // Send boot command
        #[rustfmt::skip]
		qemu.vnc.run(vec![
			// Initial wait
			wait!(30),
            // Wait for automatic login
			wait_screen_rect!("94a2520c082650cc01a4b5eac8719b697a4bbf63", 100, 100, 100, 100),
            enter!("sudo su -"),
            // Mount config partition and copy configuration.nix
            enter!("mkdir /goldboot"),
            enter!("mount /dev/vdb /goldboot"),
            enter!("cp /goldboot/configuration.nix /mnt/etc/nixos/configuration.nix"),
            enter!("umount /goldboot"),
			// Run install
			enter!("nixos-install"),
		])?;

        // Shutdown
        qemu.shutdown_wait()?;
        Ok(())
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
struct ConfigurationPath(PathBuf);

impl ConfigurationPath {
    fn load(&self) -> Result<Vec<u8>> {
        if self.0.starts_with("http") {
            todo!()
        }

        let bytes = std::fs::read(&self.0)?;
        Ok(bytes)
    }
}

impl Prompt for ConfigurationPath {
    fn prompt(&mut self, _: &Builder) -> Result<()> {
        todo!()
    }
}
