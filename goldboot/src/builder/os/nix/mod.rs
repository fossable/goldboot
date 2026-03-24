use anyhow::Result;
use goldboot_image::ImageArch;
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;
use std::{collections::HashMap, path::PathBuf};
use validator::Validate;

use crate::{
    builder::{
        Builder,
        options::{arch::Arch, iso::Iso, size::Size},
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
#[goldboot_macros::Os(architectures(Amd64, Arm64))]
#[derive(Clone, Serialize, Deserialize, Validate, Debug, SmartDefault, goldboot_macros::Prompt)]
pub struct Nix {
    #[default(Arch(ImageArch::Amd64))]
    pub arch: Arch,
    pub size: Size,

    /// Path to configuration.nix to install
    #[default(ConfigurationPath("configuration.nix".parse().unwrap()))]
    pub configuration: ConfigurationPath,

    #[default(Iso {
        url: "https://channels.nixos.org/nixos-24.11/latest-nixos-minimal-x86_64-linux.iso".parse().unwrap(),
        checksum: Some("sha256:acdcf8239f64e5acd20cf49c63f83e4c1b823b31d9f669033b48876b29b52177".to_string()),
    })]
    pub iso: Iso,
}

impl BuildImage for Nix {
    fn build(&self, worker: &Builder) -> Result<()> {
        let mut qemu = QemuBuilder::new(&worker, OsCategory::Linux)
            .with_iso(&self.iso)?
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
            // Mount config drive and copy configuration.nix
            enter!("mkdir /goldboot && mount /dev/vdb /goldboot"),
            enter!("nixos-generate-config --root /mnt"),
            enter!("cp /goldboot/configuration.nix /mnt/etc/nixos/configuration.nix"),
            enter!("umount /goldboot"),
			// Run install
			enter!("nixos-install --no-root-passwd"),
		])?;

        // Shutdown
        qemu.shutdown_wait()?;
        Ok(())
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ConfigurationPath(PathBuf);

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
