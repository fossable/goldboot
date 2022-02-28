use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Clone, Serialize, Deserialize, Validate, Default)]
pub struct PackerTemplate {
    pub builders: Vec<QemuBuilder>,
    pub provisioners: Vec<PackerProvisioner>,
}

#[derive(Clone, Serialize, Deserialize, Validate, Default)]
pub struct QemuBuilder {
    pub boot_command: Vec<String>,
    pub boot_wait: String,
    pub communicator: String,
    pub format: String,
    pub headless: bool,
    pub iso_checksum: Option<String>,
    pub iso_url: Option<String>,
    pub output_directory: Option<String>,
    pub qemuargs: Option<Vec<Vec<String>>>,
    pub r#type: String,
    pub shutdown_command: String,
    pub ssh_password: Option<String>,
    pub ssh_username: Option<String>,
    pub ssh_wait_timeout: Option<String>,
    pub vm_name: Option<String>,
    pub winrm_insecure: Option<bool>,
    pub winrm_password: Option<String>,
    pub winrm_timeout: Option<String>,
    pub winrm_username: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Validate, Default)]
pub struct PackerProvisioner {
	pub extra_arguments: Vec<String>,
	pub playbook_file: Option<String>,
	pub r#type: String,
	pub scripts: Vec<String>,
	pub use_proxy: Option<bool>,
	pub user: Option<String>,
}
