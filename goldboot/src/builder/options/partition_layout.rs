use anyhow::Result;
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;

use crate::{builder::Builder, cli::prompt::Prompt};

/// Disk partitioning layout. All variants target GPT and UEFI firmware.
#[derive(Clone, Serialize, Deserialize, Debug, SmartDefault)]
pub enum PartitionLayout {
    /// GPT: 512 MB EFI system partition + ext4 root
    #[default]
    Uefi,

    /// GPT: 512 MB EFI + swap partition + ext4 root
    UefiWithSwap {
        /// Swap partition size in MiB
        #[default = 4096]
        swap_size_mib: u64,
    },

    /// GPT: 512 MB EFI + LUKS2-encrypted ext4 root
    UefiLuks { passphrase: String },
}

impl PartitionLayout {
    /// Shell commands to partition, format, and mount `device` at `/mnt`.
    /// Used by builders that operate from a live shell (e.g. NixOS).
    pub fn mount_commands(&self, device: &str) -> Vec<String> {
        match self {
            PartitionLayout::Uefi => vec![
                format!("sgdisk -n1:0:+512M -t1:ef00 -n2:0:0 -t2:8300 {device}"),
                format!("mkfs.fat -F32 {device}1"),
                format!("mkfs.ext4 {device}2"),
                format!("mount {device}2 /mnt"),
                "mkdir -p /mnt/boot".into(),
                format!("mount {device}1 /mnt/boot"),
            ],
            PartitionLayout::UefiWithSwap { swap_size_mib } => vec![
                format!(
                    "sgdisk -n1:0:+512M -t1:ef00 -n2:0:+{swap_size_mib}M -t2:8200 -n3:0:0 -t3:8300 {device}"
                ),
                format!("mkfs.fat -F32 {device}1"),
                format!("mkswap {device}2"),
                format!("swapon {device}2"),
                format!("mkfs.ext4 {device}3"),
                format!("mount {device}3 /mnt"),
                "mkdir -p /mnt/boot".into(),
                format!("mount {device}1 /mnt/boot"),
            ],
            PartitionLayout::UefiLuks { passphrase } => vec![
                format!("sgdisk -n1:0:+512M -t1:ef00 -n2:0:0 -t2:8309 {device}"),
                format!("mkfs.fat -F32 {device}1"),
                format!("echo -n '{passphrase}' | cryptsetup luksFormat {device}2 -"),
                format!("echo -n '{passphrase}' | cryptsetup luksOpen {device}2 root -"),
                "mkfs.ext4 /dev/mapper/root".into(),
                "mount /dev/mapper/root /mnt".into(),
                "mkdir -p /mnt/boot".into(),
                format!("mount {device}1 /mnt/boot"),
            ],
        }
    }
}

impl Prompt for PartitionLayout {
    fn prompt(&mut self, _: &Builder) -> Result<()> {
        use dialoguer::{Input, Password, Select};
        let theme = crate::cli::cmd::init::theme();

        let labels = [
            "UEFI (EFI + ext4 root)",
            "UEFI with swap (EFI + swap + ext4 root)",
            "UEFI with LUKS (EFI + encrypted ext4 root)",
        ];

        let selection = Select::with_theme(&theme)
            .with_prompt("Partition layout")
            .items(labels)
            .default(0)
            .interact()?;

        *self = match selection {
            0 => PartitionLayout::Uefi,
            1 => PartitionLayout::UefiWithSwap {
                swap_size_mib: Input::with_theme(&theme)
                    .with_prompt("Swap partition size (MiB)")
                    .default(4096)
                    .interact_text()?,
            },
            _ => PartitionLayout::UefiLuks {
                passphrase: Password::with_theme(&theme)
                    .with_prompt("LUKS passphrase")
                    .interact()?,
            },
        };

        Ok(())
    }
}
