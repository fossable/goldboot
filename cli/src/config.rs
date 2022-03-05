use crate::qemu::QemuConfig;
use anyhow::Result;
use log::debug;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::default::Default;
use std::error::Error;
use std::fmt;
use std::fs;
use std::str::FromStr;
use validator::Validate;

#[derive(Clone, Serialize, Deserialize, Validate, Default)]
pub struct Config {
    pub name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    pub base: Option<Profile>,

    pub provisioners: Vec<Provisioner>,

    pub qemu: QemuConfig,

    /// The installation media URL
    pub iso_url: String,

    /// A hash of the installation media
    pub iso_checksum: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub arch: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub packer_template: Option<String>,

    /// The amount of memory to allocate to the VM
    pub memory: String,

    /// The size of the disk to attach to the VM
    pub disk_size: String,

    #[serde(flatten)]
    pub profile: HashMap<String, String>,
}

impl Config {
    pub fn load() -> Result<Config> {
        debug!("Loading config");

        // Read config from working directory
        let config: Config = serde_json::from_slice(&fs::read("goldboot.json").unwrap()).unwrap();
        Ok(config)
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum Profile {
    ArchLinux,
    Windows10,
    PopOs2104,
    PopOs2110,
}

#[derive(Debug)]
pub struct ProfileParseErr;

impl Error for ProfileParseErr {}

impl fmt::Display for ProfileParseErr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ArchLinux, Windows10, PopOs2104, PopOs2110")
    }
}

impl FromStr for Profile {
    type Err = ProfileParseErr;

    fn from_str(string: &str) -> Result<Self, <Self as FromStr>::Err> {
        match string {
            "ArchLinux" => Ok(Profile::ArchLinux),
            "Windows10" => Ok(Profile::Windows10),
            "PopOs2104" => Ok(Profile::PopOs2104),
            "PopOs2110" => Ok(Profile::PopOs2110),
            _ => Err(ProfileParseErr {}),
        }
    }
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
    pub playbook: String,
}

#[derive(Clone, Serialize, Deserialize, Validate)]
pub struct ShellProvisioner {
    pub script: String,

    pub inline: Vec<String>,
}
