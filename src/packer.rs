use crate::build_headless_debug;
use serde::Serialize;
use validator::Validate;

#[derive(Clone, Serialize, Validate, Default, Debug)]
pub struct PackerTemplate {
    pub builders: Vec<QemuBuilder>,
    pub provisioners: Vec<PackerProvisioner>,
}

#[derive(Clone, Serialize, Validate, Default, Debug)]
pub struct QemuBuilder {
    pub boot_command: Vec<String>,
    pub boot_wait: String,
    pub communicator: String,
    pub disk_compression: bool,
    pub disk_size: String,
    pub format: String,
    pub headless: bool,
    pub iso_checksum: String,
    pub iso_url: String,
    pub memory: String,
    pub output_directory: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qemu_binary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qemuargs: Option<Vec<Vec<String>>>,
    pub r#type: String,
    pub shutdown_command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssh_password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssh_username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssh_wait_timeout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vm_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub winrm_insecure: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub winrm_password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub winrm_timeout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub winrm_username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub floppy_files: Option<Vec<String>>,
    pub disk_interface: String,
}

impl QemuBuilder {
    pub fn new() -> QemuBuilder {
        let mut builder = QemuBuilder::default();
        builder.format = String::from("qcow2");
        builder.headless = build_headless_debug();
        builder.r#type = String::from("qemu");
        builder.disk_compression = true;

        return builder;
    }
}

#[derive(Clone, Serialize, Validate, Default, Debug)]
pub struct PackerProvisioner {
    pub extra_arguments: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub playbook_file: Option<String>,
    pub r#type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scripts: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_proxy: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
}

pub mod bootcmds {
    macro_rules! enter {
        ($text:expr) => {
            format!("{}<enter><wait>", $text)
        };
        () => {
            format!("<enter><wait>")
        };
    }

    macro_rules! spacebar {
        () => {
            "<spacebar><wait>".to_string()
        };
    }

    macro_rules! tab {
        ($text:expr) => {
            format!("{}<tab><wait>", $text)
        };
        () => {
            format!("<tab><wait>")
        };
    }

    macro_rules! wait {
        ($duration:expr) => {
            format!("<wait{}s>", $duration)
        };
    }

    macro_rules! input {
        ($text:expr) => {
            format!("{}", $text)
        };
    }

    macro_rules! leftSuper {
        () => {
            "<leftSuper><wait>".to_string()
        };
    }

    pub(crate) use enter;
    pub(crate) use input;
    pub(crate) use leftSuper;
    pub(crate) use spacebar;
    pub(crate) use tab;
    pub(crate) use wait;
}

// TODO install plugin function
