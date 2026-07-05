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
        steps::{PostStep, PreStep},
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

    /// Steps that run on the host before the VM boots (e.g. rendering the
    /// configuration from a template).
    #[serde(default)]
    pub pre_steps: Vec<PreStep>,

    /// Steps that run over SSH against the installed system after the build
    /// completes. Requires the installed system to run sshd (the example's
    /// `configuration.nix` enables `services.openssh`).
    #[serde(default)]
    pub post_steps: Vec<PostStep>,
}

impl BuildImage for Nix {
    fn build(&self, worker: &Builder) -> Result<()> {
        let has_post_steps = !self.post_steps.is_empty();

        // TODO enable serial instead of VNC
        let qemu_builder = QemuBuilder::new(worker, OsCategory::Linux)
            .with_iso(&self.iso)?
            // Forward the host SSH port to the installed system's sshd so
            // post-steps can connect after the VM reboots.
            .forward_ssh(22);
        let public_key = qemu_builder.ssh_public_key()?;

        // The config drive (/dev/vdb) carries the configuration and the
        // goldboot public key used to reach the installed system over SSH.
        let mut qemu = qemu_builder
            .drive_files(HashMap::from([
                (
                    "configuration.nix".to_string(),
                    self.configuration.load(&worker.effective_context_dir)?,
                ),
                ("public_key".to_string(), public_key),
            ]))?
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
            enter!("nixos-install --no-root-passwd"),
        ]);

        if has_post_steps {
            // Authorize the goldboot key for root, then reboot into the
            // installed system so post-steps can run against it over SSH.
            cmds.extend(vec![
                enter!("mkdir -p /mnt/root/.ssh"),
                enter!("cp /goldboot/public_key /mnt/root/.ssh/authorized_keys"),
                enter!("chmod 700 /mnt/root/.ssh && chmod 600 /mnt/root/.ssh/authorized_keys"),
                enter!("umount /goldboot"),
                enter!("reboot"),
            ]);
        } else {
            cmds.extend(vec![enter!("umount /goldboot"), enter!("poweroff")]);
        }

        qemu.vnc.run(cmds)?;

        if has_post_steps {
            let mut ssh = qemu.ssh("root")?;
            for step in &self.post_steps {
                step.run(&mut ssh, &worker.effective_context_dir)?;
            }
            ssh.shutdown("poweroff")?;
        }

        qemu.shutdown_wait()?;
        Ok(())
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ConfigurationPath(PathBuf);

impl ConfigurationPath {
    /// Read the configuration file from the effective context directory
    /// (pre-steps may have rendered or modified it there).
    fn load(&self, context_dir: &Path) -> Result<Vec<u8>> {
        if self.0.starts_with("http") {
            todo!()
        }

        Ok(std::fs::read(context_dir.join(&self.0))?)
    }
}

impl Prompt for ConfigurationPath {
    fn prompt(&mut self, _: &Builder) -> Result<()> {
        todo!()
    }
}
