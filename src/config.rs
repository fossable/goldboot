use crate::profile::Profile;
use crate::profiles;
use log::debug;
use rand::Rng;
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

    #[serde(skip_serializing_if = "Option::is_none", default = "default_ssh_port")]
    pub ssh_port: Option<u16>,

    #[serde(skip_serializing_if = "Option::is_none", default = "default_vnc_port")]
    pub vnc_port: Option<u16>,

    #[serde(skip_serializing_if = "Option::is_none", rename = "Alpine")]
    pub profile_alpine: Option<profiles::alpine::AlpineProfile>,

    #[serde(skip_serializing_if = "Option::is_none", rename = "ArchLinux")]
    pub profile_arch_linux: Option<profiles::arch_linux::ArchLinuxProfile>,

    #[serde(skip_serializing_if = "Option::is_none", rename = "Windows10")]
    pub profile_windows_10: Option<profiles::windows_10::Windows10Profile>,

    #[serde(skip_serializing_if = "Option::is_none", rename = "Pop!_OS")]
    pub profile_pop_os: Option<profiles::pop_os::PopOsProfile>,

    #[serde(skip_serializing_if = "Option::is_none", rename = "SteamOS")]
    pub profile_steam_os: Option<profiles::steam_os::SteamOsProfile>,

    #[serde(skip_serializing_if = "Option::is_none", rename = "SteamDeck")]
    pub profile_steam_deck: Option<profiles::steam_deck::SteamDeckProfile>,

    #[serde(skip_serializing_if = "Option::is_none", rename = "UbuntuServer")]
    pub profile_ubuntu_server: Option<profiles::ubuntu_server::UbuntuServerProfile>,
}

fn default_ssh_port() -> Option<u16> {
    Some(rand::thread_rng().gen_range(10000..11000))
}

fn default_vnc_port() -> Option<u16> {
    Some(rand::thread_rng().gen_range(5900..5999))
}

impl Config {
    /// Read config from working directory
    pub fn load() -> Result<Config, Box<dyn Error>> {
        debug!("Loading config");
        Ok(serde_json::from_slice(&fs::read("goldboot.json")?)?)
    }

    pub fn get_profiles(&self) -> Vec<&dyn Profile> {
        let mut profiles: Vec<&dyn Profile> = Vec::new();

        if let Some(profile) = &self.profile_alpine {
            profiles.push(profile);
        }
        if let Some(profile) = &self.profile_arch_linux {
            profiles.push(profile);
        }
        if let Some(profile) = &self.profile_windows_10 {
            profiles.push(profile);
        }
        if let Some(profile) = &self.profile_pop_os {
            profiles.push(profile);
        }
        if let Some(profile) = &self.profile_steam_os {
            profiles.push(profile);
        }
        if let Some(profile) = &self.profile_steam_deck {
            profiles.push(profile);
        }

        profiles
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
