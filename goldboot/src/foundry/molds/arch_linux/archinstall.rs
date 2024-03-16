use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

use crate::foundry::options::hostname::Hostname;
use crate::foundry::options::unix_account::RootPassword;

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
    pub dev_path: String,
    pub partn: i32,
    pub partuuid: String,
    pub uuid: String,
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
pub struct Nic {
    pub dhcp: bool,
    pub dns: Option<Value>,
    pub gateway: Option<Value>,
    pub iface: Option<Value>,
    pub ip: Option<Value>,
    #[serde(rename = "type")]
    pub type_field: String,
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
pub struct ArchinstallConfig {
    #[serde(rename = "additional-repositories")]
    pub additional_repositories: Vec<Value>,
    #[serde(rename = "archinstall-language")]
    pub archinstall_language: String,
    pub audio_config: Option<AudioConfig>,
    pub bootloader: String,
    pub config_version: String,
    pub debug: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disk_encryption: Option<DiskEncryption>,
    pub dry_run: bool,
    pub disk_config: DiskLayoutConfiguration,
    pub hostname: String,
    pub kernels: Vec<String>,
    #[serde(rename = "keyboard-layout")]
    pub keyboard_layout: String,
    #[serde(rename = "mirror-region")]
    pub mirror_region: String,
    pub nic: Nic,
    pub no_pkg_lookups: bool,
    pub ntp: bool,
    pub offline: bool,
    pub packages: Vec<String>,
    #[serde(rename = "parallel downloads")]
    pub parallel_downloads: i64,
    pub profile_config: Option<ProfileConfig>,
    pub silent: bool,
    pub swap: bool,
    #[serde(rename = "sys-encoding")]
    pub sys_encoding: String,
    #[serde(rename = "sys-language")]
    pub sys_language: String,
    pub timezone: String,
    pub version: String,
}

impl From<&super::ArchLinux> for ArchinstallConfig {
    fn from(value: &super::ArchLinux) -> Self {
        Self {
            additional_repositories: vec![],
            archinstall_language: "English".to_string(),
            audio_config: None,
            bootloader: "grub".to_string(),
            config_version: "2.6.0".to_string(),
            debug: true,
            disk_encryption: None,
            dry_run: false,
            disk_config: DiskLayoutConfiguration {
                config_type: "manual_partitioning".to_string(),
                device_modifications: vec![DeviceModification {
                    device: "/dev/vda".to_string(),
                    partitions: vec![
                        PartitionModification {
                            flags: vec!["Boot".to_string()],
                            fs_type: "fat32".to_string(),
                            size: Size {
                                sector_size: SectorSize {
                                    value: 512,
                                    unit: "B".to_string(),
                                },
                                unit: "MiB".to_string(),
                                value: 512,
                            },
                            mount_options: vec![],
                            mountpoint: "/boot".to_string(),
                            start: Size {
                                sector_size: SectorSize {
                                    value: 512,
                                    unit: "B".to_string(),
                                },
                                unit: "MiB".to_string(),
                                value: 1,
                            },
                            status: "create".to_string(),
                            type_field: "primary".to_string(),
                            dev_path: "/dev/vda1".to_string(),
                            partn: 1,
                            partuuid: "".to_string(),
                            uuid: Uuid::new_v4().to_string(),
                            obj_id: Uuid::new_v4().to_string(),
                        },
                        PartitionModification {
                            flags: vec![],
                            fs_type: "ext4".to_string(),
                            size: Size {
                                sector_size: SectorSize {
                                    value: 512,
                                    unit: "B".to_string(),
                                },
                                unit: "MiB".to_string(),
                                value: 5120,
                            },
                            mount_options: vec![],
                            mountpoint: "/".to_string(),
                            start: Size {
                                sector_size: SectorSize {
                                    value: 512,
                                    unit: "B".to_string(),
                                },
                                unit: "MiB".to_string(),
                                value: 513,
                            },
                            status: "create".to_string(),
                            type_field: "primary".to_string(),
                            dev_path: "/dev/vda2".to_string(),
                            partn: 2,
                            partuuid: "".to_string(),
                            uuid: Uuid::new_v4().to_string(),
                            obj_id: Uuid::new_v4().to_string(),
                        },
                    ],
                    wipe: false,
                }],
            },
            hostname: value
                .hostname
                .clone()
                .unwrap_or(Hostname::default())
                .to_string(),
            kernels: vec!["linux".to_string()],
            keyboard_layout: "us".to_string(),
            mirror_region: "Worldwide".to_string(),
            nic: Nic {
                dhcp: true,
                dns: None,
                gateway: None,
                iface: None,
                ip: None,
                type_field: "nm".to_string(),
            },
            no_pkg_lookups: false,
            ntp: true,
            offline: false,
            packages: value.packages.clone().unwrap_or_default().packages,
            parallel_downloads: 0,
            profile_config: None,
            silent: true,
            swap: false,
            sys_encoding: "utf-8".to_string(),
            sys_language: "en_US".to_string(),
            timezone: "UTC".to_string(),
            version: "2.6.0".to_string(),
        }
    }
}
