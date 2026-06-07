use anyhow::Result;
use goldboot_image::ImageArch;
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use validator::Validate;

use crate::{
    builder::{
        Builder,
        options::{
            arch::Arch, iso::Iso, minimum_size::MinimumSize, partition_layout::PartitionLayout,
        },
        qemu::{OsCategory, QemuBuilder},
    },
    cli::prompt::Prompt,
    enter, wait, wait_text,
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
    pub minimum_size: MinimumSize,

    /// Path to configuration.nix to install
    #[default(ConfigurationPath("configuration.nix".parse().unwrap()))]
    pub configuration: ConfigurationPath,

    #[default(Iso {
        url: "https://channels.nixos.org/nixos-24.11/latest-nixos-minimal-x86_64-linux.iso".parse().unwrap(),
        checksum: Some("sha256:acdcf8239f64e5acd20cf49c63f83e4c1b823b31d9f669033b48876b29b52177".to_string()),
    })]
    pub iso: Iso,

    #[default(PartitionLayout::Uefi)]
    pub partition_layout: PartitionLayout,
}

impl BuildImage for Nix {
    fn build(&self, worker: &Builder) -> Result<()> {
        // TODO enable serial instead of VNC
        let mut qemu = QemuBuilder::new(worker, OsCategory::Linux)
            .with_iso(&self.iso)?
            .drive_files(HashMap::from([(
                "configuration.nix".to_string(),
                self.configuration.load(&worker.context_dir)?,
            )]))?
            .start()?;

        let mut cmds = vec![
            wait!(15),
            wait_text!("nixos login: nixos .automatic login."),
            enter!("sudo su -"),
            enter!("mkdir /goldboot && mount /dev/vdb /goldboot"),
        ];

        for cmd in self.partition_layout.mount_commands("/dev/vda") {
            cmds.push(enter!(cmd));
        }

        cmds.extend(vec![
            enter!("nixos-generate-config --root /mnt"),
            enter!("cp /goldboot/configuration.nix /mnt/etc/nixos/configuration.nix"),
            enter!("umount /goldboot"),
            enter!("nixos-install --no-root-passwd"),
            enter!("poweroff"),
        ]);

        qemu.vnc.run(cmds)?;

        qemu.shutdown_wait()?;
        Ok(())
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ConfigurationPath(PathBuf);

impl ConfigurationPath {
    fn load(&self, context_dir: &Path) -> Result<Vec<u8>> {
        if self.0.starts_with("http") {
            todo!()
        }

        let path = if self.0.is_absolute() {
            self.0.clone()
        } else {
            context_dir.join(&self.0)
        };
        let bytes = std::fs::read(&path)?;
        Ok(bytes)
    }
}

impl Prompt for ConfigurationPath {
    fn prompt(&mut self, _: &Builder) -> Result<()> {
        todo!()
    }
}
