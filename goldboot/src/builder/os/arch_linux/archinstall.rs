use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

use crate::builder::options::partition_layout::PartitionLayout;
use crate::builder::options::root_password::RootPassword;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArchinstallCredentials {
    #[serde(rename = "!root-password")]
    pub root_password: String,
    #[serde(
        rename = "!encryption-password",
        skip_serializing_if = "Option::is_none"
    )]
    pub encryption_password: Option<String>,
    #[serde(rename = "!users", skip_serializing_if = "Option::is_none")]
    pub users: Option<Vec<User>>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct User {
    pub username: String,
    #[serde(rename = "!password")]
    pub password: String,
    pub sudo: bool,
}

impl From<&super::ArchLinux> for ArchinstallCredentials {
    fn from(value: &super::ArchLinux) -> Self {
        let root_password = match &value.root_password {
            RootPassword::Plaintext(p) => p.to_string(),
            RootPassword::PlaintextEnv(name) => {
                std::env::var(name).expect("environment variable not found")
            }
        };
        Self {
            root_password,
            encryption_password: match &value.partition_layout {
                PartitionLayout::UefiLuks { passphrase } => Some(passphrase.clone()),
                _ => None,
            },
            users: value.users.as_ref().map(|users| {
                users
                    .0
                    .iter()
                    .map(|u| User {
                        username: u.username.clone(),
                        password: u.password.clone(),
                        sudo: u.sudo,
                    })
                    .collect()
            }),
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioConfig {
    pub audio: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BootloaderConfig {
    pub bootloader: String,
    pub uki: bool,
    pub removable: bool,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiskLayoutConfiguration {
    pub config_type: String,
    pub device_modifications: Vec<DeviceModification>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeviceModification {
    pub device: String,
    pub partitions: Vec<PartitionModification>,
    pub wipe: bool,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PartitionModification {
    pub btrfs: Vec<Value>,
    pub flags: Vec<String>,
    pub fs_type: String,
    pub obj_id: String,
    pub size: Size,
    pub mount_options: Vec<Value>,
    pub mountpoint: String,
    pub start: Size,
    pub status: String,
    #[serde(rename = "type")]
    pub type_field: String,
    pub dev_path: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SectorSize {
    pub value: u64,
    pub unit: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Size {
    pub sector_size: SectorSize,
    pub unit: String,
    pub value: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NetworkConfig {
    #[serde(rename = "type")]
    pub type_field: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub nics: Vec<Nic>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Nic {
    pub dhcp: bool,
    pub dns: Option<Vec<String>>,
    pub gateway: Option<String>,
    pub iface: Option<String>,
    pub ip: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocaleConfig {
    pub kb_layout: String,
    pub sys_enc: String,
    pub sys_lang: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MirrorConfig {
    pub mirror_regions: Value,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub custom_servers: Vec<Value>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileConfig {
    pub gfx_driver: String,
    pub greeter: String,
    pub profile: Profile,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub details: Vec<String>,
    pub main: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiskEncryption {
    pub partitions: Vec<String>,
    pub encryption_type: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Swap {
    pub enabled: bool,
    pub algorithm: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArchinstallConfig {
    #[serde(rename = "archinstall-language")]
    pub archinstall_language: String,
    pub audio_config: Option<AudioConfig>,
    pub bootloader_config: BootloaderConfig,
    pub debug: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disk_encryption: Option<DiskEncryption>,
    pub disk_config: DiskLayoutConfiguration,
    pub hostname: String,
    pub kernels: Vec<String>,
    pub locale_config: LocaleConfig,
    pub mirror_config: MirrorConfig,
    pub network_config: NetworkConfig,
    pub no_pkg_lookups: bool,
    pub ntp: bool,
    pub offline: bool,
    pub packages: Vec<String>,
    #[serde(rename = "parallel downloads")]
    pub parallel_downloads: i64,
    pub profile_config: Option<ProfileConfig>,
    pub silent: bool,
    pub swap: Swap,
    pub timezone: String,
}

/// Converts a `PartitionLayout` into the archinstall disk_config and optional
/// disk_encryption structures. The root partition's obj_id is shared between
/// both so that archinstall correctly associates the encryption entry with the
/// right partition.
fn disk_config_from(layout: &PartitionLayout) -> (DiskLayoutConfiguration, Option<DiskEncryption>) {
    fn sector_size() -> SectorSize {
        SectorSize {
            value: 512,
            unit: "B".to_string(),
        }
    }
    fn mib(value: i64, offset: i64) -> (Size, Size) {
        (
            Size {
                sector_size: sector_size(),
                unit: "MiB".to_string(),
                value: offset,
            },
            Size {
                sector_size: sector_size(),
                unit: "MiB".to_string(),
                value,
            },
        )
    }

    let efi_obj_id = Uuid::new_v4().to_string();
    let (efi_start, efi_size) = mib(512, 1);
    let efi_part = PartitionModification {
        btrfs: vec![],
        flags: vec!["boot".to_string(), "esp".to_string()],
        fs_type: "fat32".to_string(),
        obj_id: efi_obj_id,
        size: efi_size,
        mount_options: vec![],
        mountpoint: "/boot".to_string(),
        start: efi_start,
        status: "create".to_string(),
        type_field: "primary".to_string(),
        dev_path: Some("/dev/vda1".to_string()),
    };

    match layout {
        PartitionLayout::Uefi => {
            let (root_start, root_size) = mib(-1, 513);
            let root_part = PartitionModification {
                btrfs: vec![],
                flags: vec![],
                fs_type: "ext4".to_string(),
                obj_id: Uuid::new_v4().to_string(),
                size: root_size,
                mount_options: vec![],
                mountpoint: "/".to_string(),
                start: root_start,
                status: "create".to_string(),
                type_field: "primary".to_string(),
                dev_path: Some("/dev/vda2".to_string()),
            };
            (
                DiskLayoutConfiguration {
                    config_type: "manual_partitioning".to_string(),
                    device_modifications: vec![DeviceModification {
                        device: "/dev/vda".to_string(),
                        wipe: true,
                        partitions: vec![efi_part, root_part],
                    }],
                },
                None,
            )
        }
        PartitionLayout::UefiWithSwap { swap_size_mib } => {
            let swap_size_mib = *swap_size_mib as i64;
            let (swap_start, swap_size) = mib(swap_size_mib, 513);
            let swap_part = PartitionModification {
                btrfs: vec![],
                flags: vec!["swap".to_string()],
                fs_type: "swap".to_string(),
                obj_id: Uuid::new_v4().to_string(),
                size: swap_size,
                mount_options: vec![],
                mountpoint: String::new(),
                start: swap_start,
                status: "create".to_string(),
                type_field: "primary".to_string(),
                dev_path: Some("/dev/vda2".to_string()),
            };
            let (root_start, root_size) = mib(-1, 513 + swap_size_mib);
            let root_part = PartitionModification {
                btrfs: vec![],
                flags: vec![],
                fs_type: "ext4".to_string(),
                obj_id: Uuid::new_v4().to_string(),
                size: root_size,
                mount_options: vec![],
                mountpoint: "/".to_string(),
                start: root_start,
                status: "create".to_string(),
                type_field: "primary".to_string(),
                dev_path: Some("/dev/vda3".to_string()),
            };
            (
                DiskLayoutConfiguration {
                    config_type: "manual_partitioning".to_string(),
                    device_modifications: vec![DeviceModification {
                        device: "/dev/vda".to_string(),
                        wipe: true,
                        partitions: vec![efi_part, swap_part, root_part],
                    }],
                },
                None,
            )
        }
        PartitionLayout::UefiLuks { .. } => {
            let root_obj_id = Uuid::new_v4().to_string();
            let (root_start, root_size) = mib(-1, 513);
            let root_part = PartitionModification {
                btrfs: vec![],
                flags: vec![],
                fs_type: "ext4".to_string(),
                obj_id: root_obj_id.clone(),
                size: root_size,
                mount_options: vec![],
                mountpoint: "/".to_string(),
                start: root_start,
                status: "create".to_string(),
                type_field: "primary".to_string(),
                dev_path: Some("/dev/vda2".to_string()),
            };
            (
                DiskLayoutConfiguration {
                    config_type: "manual_partitioning".to_string(),
                    device_modifications: vec![DeviceModification {
                        device: "/dev/vda".to_string(),
                        wipe: true,
                        partitions: vec![efi_part, root_part],
                    }],
                },
                Some(DiskEncryption {
                    partitions: vec![root_obj_id],
                    encryption_type: "luks2".to_string(),
                }),
            )
        }
    }
}

impl From<&super::ArchLinux> for ArchinstallConfig {
    fn from(value: &super::ArchLinux) -> Self {
        use super::{ArchLinuxAudio, ArchLinuxBootloaderKind};

        let (disk_config, disk_encryption) = disk_config_from(&value.partition_layout);

        Self {
            archinstall_language: "English".to_string(),
            audio_config: value.audio.as_ref().map(|a| AudioConfig {
                audio: match a {
                    ArchLinuxAudio::Pipewire => "pipewire".to_string(),
                    ArchLinuxAudio::Pulseaudio => "pulseaudio".to_string(),
                },
            }),
            bootloader_config: BootloaderConfig {
                bootloader: match value.bootloader.kind {
                    ArchLinuxBootloaderKind::Grub => "Grub".to_string(),
                    ArchLinuxBootloaderKind::Systemd => "Systemd-boot".to_string(),
                },
                uki: value.bootloader.uki,
                removable: value.bootloader.removable,
            },
            debug: true,
            disk_encryption,
            disk_config,
            hostname: value.hostname.0.clone(),
            kernels: value.kernels.0.clone(),
            locale_config: LocaleConfig {
                kb_layout: value.locale.keyboard.clone(),
                sys_enc: value.locale.encoding.clone(),
                sys_lang: value.locale.language.clone(),
            },
            mirror_config: MirrorConfig {
                mirror_regions: serde_json::json!({}),
                custom_servers: value
                    .mirrorlist
                    .as_ref()
                    .map(|m| m.mirrors.iter().map(|s| serde_json::json!(s)).collect())
                    .unwrap_or_default(),
            },
            network_config: NetworkConfig {
                type_field: "nm".to_string(),
                nics: vec![],
            },
            no_pkg_lookups: false,
            ntp: value.ntp.0,
            offline: false,
            packages: value.packages.clone().unwrap_or_default().0,
            parallel_downloads: value.parallel_downloads.0 as i64,
            profile_config: value.profile.as_ref().map(|p| ProfileConfig {
                gfx_driver: p.gfx_driver.clone().unwrap_or_default(),
                greeter: p.greeter.clone().unwrap_or_default(),
                profile: Profile {
                    main: p.name.clone(),
                    details: p.details.clone(),
                },
            }),
            silent: true,
            swap: Swap {
                enabled: value.swap.enabled,
                algorithm: value.swap.algorithm.clone(),
            },
            timezone: value.timezone.0.clone(),
        }
    }
}
