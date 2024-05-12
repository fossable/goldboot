use anyhow::Result;
use dialoguer::theme::Theme;
use goldboot_image::ImageArch;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt::Display};
use strum::{Display, EnumIter, IntoEnumIterator};
use validator::Validate;

use crate::{
    cli::prompt::{Prompt, PromptNew},
    enter,
    foundry::{
        options::{hostname::Hostname, unix_account::RootPassword},
        qemu::{OsCategory, QemuBuilder},
        sources::ImageSource,
        Foundry, FoundryWorker,
    },
    wait, wait_screen_rect,
};

use super::{CastImage, DefaultSource};

/// Produces [NixOS](https://www.nixos.org) images.
#[derive(Clone, Serialize, Deserialize, Validate, Debug, Default)]
pub struct Nix {
    pub config: String,
}

impl Nix {
    fn load_config(&self) -> Result<Vec<u8>> {
        if self.config.starts_with("http") {
            todo!()
        }

        let bytes = std::fs::read(&self.config)?;
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
    fn prompt(&mut self, _foundry: &Foundry, _theme: Box<dyn Theme>) -> Result<()> {
        Ok(())
    }
}

impl CastImage for Nix {
    fn cast(&self, worker: &FoundryWorker) -> Result<()> {
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
