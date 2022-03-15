use crate::{profiles};
use log::debug;
use serde::{Deserialize, Serialize};
use std::{default::Default, error::Error, fs};
use validator::Validate;

/// The global configuration
#[derive(Clone, Serialize, Deserialize, Validate, Default)]
pub struct Config {
    /// The image name
    #[validate(length(min = 1))]
    pub name: String,

    /// An image description
    #[validate(length(max = 4096))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub arch: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub packer_template: Option<String>,

    /// The amount of memory to allocate to the VM
    pub memory: String,

    /// The size of the disk to attach to the VM
    pub disk_size: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub nvme: Option<bool>,

    pub qemuargs: Vec<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub ArchLinux: Option<profiles::arch_linux::ArchLinuxProfile>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub Windows10: Option<profiles::windows_10::Windows10Profile>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub PopOs: Option<profiles::pop_os::PopOsProfile>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub SteamOs: Option<profiles::steam_os::SteamOsProfile>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub SteamDeck: Option<profiles::steam_deck::SteamDeckProfile>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub UbuntuServer: Option<profiles::ubuntu_server::UbuntuServerProfile>,
}

impl Config {
    /// Read config from working directory
    pub fn load() -> Result<Config, Box<dyn Error>> {
        debug!("Loading config");
        Ok(serde_json::from_slice(&fs::read("goldboot.json")?)?)
    }
}

#[derive(Clone, Serialize, Deserialize, Validate)]
pub struct Partition {
    pub r#type: String,
    pub size: String,
    pub label: String,
    pub format: String,
}

/// A generic provisioner
#[derive(Clone, Serialize, Deserialize, Validate)]
pub struct Provisioner {
    pub r#type: String,

    #[serde(flatten)]
    pub ansible: AnsibleProvisioner,

    #[serde(flatten)]
    pub shell: ShellProvisioner,
}

#[derive(Clone, Serialize, Deserialize, Validate)]
pub struct AnsibleProvisioner {
    pub playbook: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Validate)]
pub struct ShellProvisioner {
    pub script: Option<String>,
}
