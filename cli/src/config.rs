use crate::qemu::QemuConfig;
use anyhow::Result;
use log::debug;
use serde::{Deserialize, Serialize};
use std::default::Default;
use std::fs;
use validator::Validate;

#[derive(Clone, Serialize, Deserialize, Validate, Default)]
pub struct Config {
    pub name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    pub base: String,

    pub provisioners: Vec<Provisioner>,

    pub qemu: QemuConfig,

    pub user: User,

    pub iso_url: Option<String>,

    pub iso_checksum: Option<String>,
}

impl Config {

    pub fn load() -> Result<Config> {
        debug!("Loading config");

        // Read config from working directory
        let config: Config = serde_json::from_slice(&fs::read("goldboot.json").unwrap()).unwrap();
        Ok(config)
    }
}

#[derive(Clone, Serialize, Deserialize, Validate, Default)]
pub struct User {

    pub username: String,
    pub password: String,
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
