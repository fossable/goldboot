use std::{path::PathBuf};
use crate::build_headless_debug;
use crate::config::Provisioner;
use serde::Serialize;
use validator::Validate;
use sha1::{Sha1, Digest};

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

    pub qemuargs: Vec<Vec<String>>,
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

    /// Locate the ISO local path
    pub fn iso_path(&self) -> PathBuf {
        let hash = hex::encode(Sha1::new().chain_update(&self.iso_url).finalize());
        if cfg!(target_os = "linux") {
            PathBuf::from(format!("/home/{}/.cache/packer/{}.iso", whoami::username(), hash))
        } else {
            panic!("Unsupported platform");
        }
    }
}

#[derive(Clone, Serialize, Validate, Default, Debug)]
pub struct PackerProvisioner {
    pub r#type: String,

    #[serde(flatten)]
    pub ansible: Option<AnsiblePackerProvisioner>,

    #[serde(flatten)]
    pub shell: Option<ShellPackerProvisioner>,
}

impl From<&Provisioner> for PackerProvisioner {
    fn from(provisioner: &Provisioner) -> Self {
        match provisioner.r#type.as_str() {
            "ansible" => PackerProvisioner {
                r#type: String::from("ansible"),
                ansible: Some(AnsiblePackerProvisioner {
                    playbook_file: Some(provisioner.ansible.playbook.as_ref().unwrap().clone()),
                    user: Some("root".into()),
                    use_proxy: Some(false),
                    extra_arguments: Some(vec![
                        String::from("-e"),
                        String::from("ansible_winrm_scheme=http"),
                        String::from("-e"),
                        String::from("ansible_winrm_server_cert_validation=ignore"),
                        String::from("-e"),
                        String::from("ansible_ssh_pass=root"),
                    ]),
                }),
                shell: None,
            },
            _ => panic!(""),
        }
    }
}

#[derive(Clone, Serialize, Validate, Default, Debug)]
pub struct AnsiblePackerProvisioner {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub playbook_file: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_proxy: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_arguments: Option<Vec<String>>,
}

#[derive(Clone, Serialize, Validate, Default, Debug)]
pub struct ShellPackerProvisioner {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scripts: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub expect_disconnect: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment_vars: Option<Vec<String>>,
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
