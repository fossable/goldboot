use crate::packer::{PackerTemplate, QemuBuilder};
use crate::qemu::QemuConfig;
use anyhow::Result;
use log::debug;
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use std::default::Default;
use std::fs;
use validator::Validate;

#[derive(RustEmbed)]
#[folder = "src/profiles"]
struct Profiles;

#[derive(Clone, Serialize, Deserialize, Validate, Default)]
pub struct Config {
    pub name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    pub base: String,

    pub provisioners: Vec<Provisioner>,

    pub qemu: QemuConfig,
}

impl Config {
    pub fn generate_packer_template(&self) -> Result<PackerTemplate> {
        debug!("Generating packer template");

        let mut template = PackerTemplate::default();

        if let Some(profile) = Profiles::get(format!("{}.json", self.base).as_str()) {
            let mut builder: QemuBuilder = serde_json::from_slice(profile.data.as_ref()).unwrap();
            builder.qemuargs = self.qemu.to_qemuargs();
            builder.r#type = String::from("qemu");
            builder.format = String::from("qcow2");
            builder.headless = true;

            template.builders.push(builder);
        } else {
            // TODO error
        }

        Ok(template)
    }

    pub fn load() -> Result<Config> {
        debug!("Loading config");

        // Read config from working directory
        let config: Config = serde_json::from_slice(&fs::read("goldboot.json").unwrap()).unwrap();
        Ok(config)
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
