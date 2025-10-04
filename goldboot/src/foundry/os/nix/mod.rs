use anyhow::Result;
use goldboot_image::ImageArch;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf};
use validator::Validate;

use crate::{
    cli::prompt::Prompt,
    enter,
    foundry::{
        Foundry, FoundryWorker,
        qemu::{OsCategory, QemuBuilder},
        sources::ImageSource,
    },
    wait, wait_screen_rect,
};

use super::{BuildImage, DefaultSource};

/// NixOS is a free and open source Linux distribution based on the Nix package
/// manager. NixOS uses an immutable design and an atomic update model. Its use
/// of a declarative configuration system allows reproducibility and
/// portability.
///
/// Upstream: https://www.nixos.org
/// Maintainer: cilki
#[derive(Clone, Serialize, Deserialize, Validate, Debug, Default)]
pub struct Nix {
    /// Path to /etc/nixos/configuration.nix
    pub configuration: PathBuf,

    /// Path to /etc/nixos/hardware-configuration.nix
    pub hardware_configuration: Option<PathBuf>,
}

impl Nix {
    fn load_config(&self) -> Result<Vec<u8>> {
        if self.configuration.starts_with("http") {
            todo!()
        }

        let bytes = std::fs::read(&self.configuration)?;
        Ok(bytes)
    }
}

impl DefaultSource for Nix {
    fn default_source(&self, _: ImageArch) -> Result<ImageSource> {
        Ok(ImageSource::Iso {
            url: "https://channels.nixos.org/nixos-23.11/latest-nixos-minimal-x86_64-linux.iso"
                .to_string(),
            checksum: None,
        })
    }
}

// TODO proc macro
impl Prompt for Nix {
    fn prompt(&mut self, _foundry: &Foundry) -> Result<()> {
        Ok(())
    }
}

impl BuildImage for Nix {
    fn build(&self, worker: &FoundryWorker) -> Result<()> {
        let mut qemu = QemuBuilder::new(&worker, OsCategory::Linux)
            .source(&worker.element.source)?
            // Add Nix config
            .drive_files(HashMap::from([(
                "configuration.nix".to_string(),
                self.load_config()?,
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
