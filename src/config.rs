use crate::qemu::QemuConfig;
use log::debug;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, default::Default, error::Error, fs};
use validator::Validate;

#[derive(Clone, Serialize, Deserialize, Validate, Default)]
pub struct Config {
    pub name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,

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
    pub fn load() -> Result<Config, Box<dyn Error>> {
        debug!("Loading config");

        // Read config from working directory
        Ok(serde_json::from_slice(&fs::read("goldboot.json")?)?)
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
    pub playbook: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Validate)]
pub struct ShellProvisioner {
    pub script: Option<String>,
}
