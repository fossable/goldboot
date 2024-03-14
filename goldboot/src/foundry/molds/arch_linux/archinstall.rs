use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

use crate::foundry::options::hostname::Hostname;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioConfig {
    pub audio: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskConfig {
    #[serde(rename = "config_type")]
    pub config_type: String,
    #[serde(rename = "device_modifications")]
    pub device_modifications: Vec<DeviceModification>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceModification {
    pub device: String,
    pub partitions: Vec<Partition>,
    pub wipe: bool,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Partition {
    pub btrfs: Vec<Value>,
    pub flags: Vec<String>,
    #[serde(rename = "fs_type")]
    pub fs_type: String,
    pub size: Size,
    #[serde(rename = "mount_options")]
    pub mount_options: Vec<Value>,
    pub mountpoint: String,
    #[serde(rename = "obj_id")]
    pub obj_id: String,
    pub start: Start,
    pub status: String,
    #[serde(rename = "type")]
    pub type_field: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Size {
    #[serde(rename = "sector_size")]
    pub sector_size: Value,
    pub unit: String,
    pub value: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Start {
    #[serde(rename = "sector_size")]
    pub sector_size: Value,
    pub unit: String,
    pub value: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MirrorRegion {}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Nic {
    pub dhcp: bool,
    pub dns: Value,
    pub gateway: Value,
    pub iface: Value,
    pub ip: Value,
    #[serde(rename = "type")]
    pub type_field: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileConfig {
    #[serde(rename = "gfx_driver")]
    pub gfx_driver: String,
    pub greeter: String,
    pub profile: Profile,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub details: Vec<String>,
    pub main: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchinstallConfig {
    #[serde(rename = "config_version")]
    pub config_version: String,
    #[serde(rename = "additional-repositories")]
    pub additional_repositories: Vec<Value>,
    #[serde(rename = "archinstall-language")]
    pub archinstall_language: String,
    #[serde(rename = "audio_config")]
    pub audio_config: AudioConfig,
    pub bootloader: String,
    pub debug: bool,
    #[serde(rename = "disk_config")]
    pub disk_config: DiskConfig,
    pub hostname: String,
    pub kernels: Vec<String>,
    #[serde(rename = "keyboard-layout")]
    pub keyboard_layout: String,
    #[serde(rename = "mirror-region")]
    pub mirror_region: MirrorRegion,
    pub nic: Nic,
    #[serde(rename = "no_pkg_lookups")]
    pub no_pkg_lookups: bool,
    pub ntp: bool,
    pub offline: bool,
    pub packages: Vec<String>,
    #[serde(rename = "parallel downloads")]
    pub parallel_downloads: i64,
    #[serde(rename = "profile_config")]
    pub profile_config: ProfileConfig,
    pub script: String,
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
            archinstall_language: "English".to_string(),
            config_version: "2.5.2".to_string(),
            keyboard_layout: "us".to_string(),
            debug: true,
            silent: true,
            ntp: true,
            kernels: vec!["linux".to_string()],
            bootloader: "grub".to_string(),
            hostname: value
                .hostname
                .clone()
                .unwrap_or(Hostname::default())
                .to_string(),
            ..Default::default()
        }
    }
}
