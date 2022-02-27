use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Clone, Serialize, Deserialize, Validate, Default)]
pub struct PackerTemplate {
    pub builders: Vec<QemuBuilder>,
    pub provisioners: Vec<Provisioner>,
}

#[derive(Clone, Serialize, Deserialize, Validate, Default)]
pub struct QemuBuilder {
    pub boot_command: Vec<String>,
    pub boot_wait: String,
    #[serde(default)]
    pub format: String,
    #[serde(default)]
    pub headless: bool,
    pub iso_checksum: String,
    pub iso_url: String,

    #[serde(default)]
    pub output_directory: String,

    #[serde(default)]
    pub qemuargs: Vec<Vec<String>>,

    #[serde(default)]
    pub r#type: String,
    pub shutdown_command: String,
    pub ssh_password: String,
    pub ssh_wait_timeout: String,
    pub ssh_username: String,
}

#[derive(Clone, Serialize, Deserialize, Validate, Default)]
pub struct Provisioner {}
