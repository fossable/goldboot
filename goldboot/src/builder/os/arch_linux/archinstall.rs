use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

use crate::builder::options::unix_account::RootPassword;

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
        match &value.root_password {
            RootPassword::Plaintext(root_password) => Self {
                root_password: root_password.to_string(),
                encryption_password: None,
                users: None,
            },
            _ => todo!(),
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

impl From<&super::ArchLinux> for ArchinstallConfig {
    fn from(value: &super::ArchLinux) -> Self {
        Self {
            archinstall_language: "English".to_string(),
            audio_config: None,
            bootloader_config: BootloaderConfig {
                bootloader: "Grub".to_string(),
                uki: false,
                removable: false,
            },
            debug: true,
            disk_encryption: None,
            disk_config: DiskLayoutConfiguration {
                config_type: "manual_partitioning".to_string(),
                device_modifications: vec![DeviceModification {
                    device: "/dev/vda".to_string(),
                    wipe: true,
                    partitions: vec![
                        PartitionModification {
                            btrfs: vec![],
                            flags: vec!["boot".to_string(), "esp".to_string()],
                            fs_type: "fat32".to_string(),
                            obj_id: Uuid::new_v4().to_string(),
                            size: Size {
                                sector_size: SectorSize { value: 512, unit: "B".to_string() },
                                unit: "MiB".to_string(),
                                value: 512,
                            },
                            mount_options: vec![],
                            mountpoint: "/boot".to_string(),
                            start: Size {
                                sector_size: SectorSize { value: 512, unit: "B".to_string() },
                                unit: "MiB".to_string(),
                                value: 1,
                            },
                            status: "create".to_string(),
                            type_field: "primary".to_string(),
                            dev_path: Some("/dev/vda1".to_string()),
                        },
                        PartitionModification {
                            btrfs: vec![],
                            flags: vec![],
                            fs_type: "ext4".to_string(),
                            obj_id: Uuid::new_v4().to_string(),
                            size: Size {
                                sector_size: SectorSize { value: 512, unit: "B".to_string() },
                                unit: "MiB".to_string(),
                                value: 5120,
                            },
                            mount_options: vec![],
                            mountpoint: "/".to_string(),
                            start: Size {
                                sector_size: SectorSize { value: 512, unit: "B".to_string() },
                                unit: "MiB".to_string(),
                                value: 513,
                            },
                            status: "create".to_string(),
                            type_field: "primary".to_string(),
                            dev_path: Some("/dev/vda2".to_string()),
                        },
                    ],
                }],
            },
            hostname: value.hostname.hostname.clone(),
            kernels: vec!["linux".to_string()],
            locale_config: LocaleConfig {
                kb_layout: "us".to_string(),
                sys_enc: "UTF-8".to_string(),
                sys_lang: "en_US".to_string(),
            },
            mirror_config: MirrorConfig {
                mirror_regions: serde_json::json!({}),
                custom_servers: vec![],
            },
            network_config: NetworkConfig {
                type_field: "nm".to_string(),
                nics: vec![],
            },
            no_pkg_lookups: false,
            ntp: true,
            offline: false,
            packages: value.packages.clone().unwrap_or_default().0,
            parallel_downloads: 0,
            profile_config: None,
            silent: true,
            swap: Swap {
                enabled: false,
                algorithm: "zstd".to_string(),
            },
            timezone: "UTC".to_string(),
        }
    }
}
