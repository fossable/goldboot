//! This module contains various common provisioners which may be included in
//! image templates. Templates may also specify their own specialized
//! provisioners for specific tasks.
//!
//! A provisioner is simply an operation to be performed on an image.

use crate::{build::BuildConfig, ssh::SshConnection, PromptMut};
use dialoguer::{theme::ColorfulTheme, Confirm, Password};
use log::{debug, info};
use serde::{Deserialize, Serialize};
use simple_error::bail;
use std::{default::Default, error::Error, path::Path, process::Command};
use strum::{Display, EnumIter};
use validator::Validate;

#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct AnsibleProvisioners {
    pub ansible: Vec<AnsibleProvisioner>,
}

/// This provisioner runs an Ansible playbook on the image remotely.
#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct AnsibleProvisioner {
    /// The playbook file
    pub playbook: String,

    /// The inventory file
    pub inventory: Option<String>,

    /// Overrides the default run order
    pub order: Option<usize>,
}

impl AnsibleProvisioner {
    pub fn run(&self, ssh: &mut SshConnection) -> Result<(), Box<dyn Error>> {
        info!("Running ansible provisioner");

        if let Some(code) = Command::new("ansible-playbook")
            .arg("--ssh-common-args")
            .arg("-o StrictHostKeyChecking=no")
            .arg("-e")
            .arg(format!("ansible_port={}", ssh.port))
            .arg("-e")
            .arg(format!("ansible_user={}", ssh.username))
            .arg("-e")
            .arg(format!("ansible_ssh_pass={}", ssh.password))
            .arg("-e")
            .arg("ansible_connection=ssh")
            .arg(&self.playbook)
            .status()
            .expect("Failed to launch ansible-playbook")
            .code()
        {
            if code != 0 {
                bail!("Provisioning failed");
            }
        }

        Ok(())
    }
}

impl PromptMut for AnsibleProvisioner {
    fn prompt(
        &mut self,
        config: &BuildConfig,
        theme: &ColorfulTheme,
    ) -> Result<(), Box<dyn Error>> {
        self.playbook = dialoguer::Input::with_theme(theme)
            .with_prompt("Enter the playbook path relative to the current directory")
            .interact()?;

        if !Path::new(&self.playbook).exists() {
            if !dialoguer::Confirm::with_theme(theme)
                .with_prompt("The path does not exist. Add anyway?")
                .interact()?
            {
                bail!("The playbook did not exist");
            }
        }

        self.validate()?;
        Ok(())
    }
}

#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct ShellProvisioners {
    pub commands: Vec<ShellProvisioner>,
}

/// This provisioner runs an inline shell command.
#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct ShellProvisioner {
    /// The inline command to run
    pub command: String,

    /// Overrides the default run order
    pub order: Option<usize>,
}

impl ShellProvisioner {
    /// Create a new shell provisioner with inline command
    pub fn new(command: &str) -> Self {
        Self {
            command: command.to_string(),
            order: None,
        }
    }

    pub fn run(&self, ssh: &mut SshConnection) -> Result<(), Box<dyn Error>> {
        info!("Running shell provisioner");

        if ssh.exec(&self.command)? != 0 {
            bail!("Provisioner failed");
        }
        Ok(())
    }
}

#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct ExecutableProvisioners {
    pub executables: Vec<ExecutableProvisioner>,
}

/// This provisioner runs an executable file on the image.
#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct ExecutableProvisioner {
    /// The path to the executable
    pub path: String,

    /// Overrides the default run order
    pub order: Option<usize>,
}

impl ExecutableProvisioner {
    pub fn run(&self, ssh: &mut SshConnection) -> Result<(), Box<dyn Error>> {
        info!("Running executable provisioner");

        if ssh.upload_exec(std::fs::read(self.path.clone())?, vec![])? != 0 {
            bail!("Provisioner failed");
        }
        Ok(())
    }
}

impl PromptMut for ExecutableProvisioner {
    fn prompt(
        &mut self,
        config: &BuildConfig,
        theme: &ColorfulTheme,
    ) -> Result<(), Box<dyn Error>> {
        self.path = dialoguer::Input::with_theme(theme)
            .with_prompt("Enter the script path relative to the current directory")
            .interact()?;

        if !Path::new(&self.path).exists() {
            if !dialoguer::Confirm::with_theme(theme)
                .with_prompt("The path does not exist. Add anyway?")
                .interact()?
            {
                bail!("The playbook did not exist");
            }
        }

        self.validate()?;
        Ok(())
    }
}

/// This provisioner changes the network hostname.
#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct HostnameProvisioner {
    // TODO validate
    pub hostname: String,
}

impl Default for HostnameProvisioner {
    fn default() -> Self {
        Self {
            hostname: String::from("goldboot"),
        }
    }
}

impl PromptMut for HostnameProvisioner {
    fn prompt(
        &mut self,
        config: &BuildConfig,
        theme: &ColorfulTheme,
    ) -> Result<(), Box<dyn Error>> {
        self.hostname = dialoguer::Input::with_theme(theme)
            .with_prompt("Enter network hostname")
            .default(config.name.clone())
            .interact()?;

        self.validate()?;
        Ok(())
    }
}

#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct TimezoneProvisioner {
    // TODO
}

impl PromptMut for TimezoneProvisioner {
    fn prompt(
        &mut self,
        config: &BuildConfig,
        theme: &ColorfulTheme,
    ) -> Result<(), Box<dyn Error>> {
        todo!()
    }
}

#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct UnixAccountProvisioners {
    pub users: Vec<UnixAccountProvisioner>,
}

impl UnixAccountProvisioners {
    /// Get the root user's password
    pub fn get_root_password(&self) -> Option<String> {
        self.users
            .iter()
            .filter(|u| u.username == "root")
            .map(|u| u.password)
            .next()
    }
}

/// This provisioner configures a UNIX-like user account.
#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct UnixAccountProvisioner {
    #[validate(length(max = 64))]
    pub username: String,

    #[validate(length(max = 64))]
    pub password: String,
}

impl PromptMut for UnixAccountProvisioner {
    fn prompt(
        &mut self,
        config: &BuildConfig,
        theme: &ColorfulTheme,
    ) -> Result<(), Box<dyn Error>> {
        self.password = dialoguer::Password::with_theme(theme)
            .with_prompt("Root password")
            .interact()?;

        self.validate()?;
        Ok(())
    }
}

impl Default for UnixAccountProvisioner {
    fn default() -> Self {
        Self {
            username: String::from("root"),
            password: crate::random_password(),
        }
    }
}

/// This provisioner configures a LUKS encrypted root filesystem
#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct LuksProvisoner {
    /// The LUKS passphrase
    pub passphrase: String,

    /// Whether the LUKS passphrase will be enrolled in a TPM
    pub tpm: bool,
}

impl PromptMut for LuksProvisoner {
    fn prompt(
        &mut self,
        config: &BuildConfig,
        theme: &ColorfulTheme,
    ) -> Result<(), Box<dyn Error>> {
        if Confirm::with_theme(theme)
            .with_prompt("Do you want to encrypt the root partition with LUKS?")
            .interact()?
        {
            self.passphrase = Password::with_theme(theme)
                .with_prompt("LUKS passphrase")
                .interact()?;
        }

        self.validate()?;
        Ok(())
    }
}

pub struct SshProvisioner {}

#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct PartitionProvisioner {
    pub total_size: String,
    // TODO
}

impl PartitionProvisioner {
    pub fn storage_size_bytes(&self) -> u64 {
        self.total_size.parse::<ubyte::ByteUnit>().unwrap().as_u64()
    }
}

pub struct DnsResolverProvisioner {}
