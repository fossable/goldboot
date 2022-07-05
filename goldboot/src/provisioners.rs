use crate::{build::BuildConfig, ssh::SshConnection, templates::Template, Promptable};
use dialoguer::{theme::ColorfulTheme, Confirm, Password};
use log::{debug, info};
use rand::Rng;
use serde::{Deserialize, Serialize};
use simple_error::bail;
use std::{default::Default, error::Error, net::TcpListener, process::Command};
use strum::{Display, EnumIter};
use validator::Validate;

#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct AnsibleProvisioner {
	pub r#type: String,

	/// The playbook file
	pub playbook: String,

	/// The inventory file
	pub inventory: Option<String>,
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

impl Promptable for AnsibleProvisioner {
	fn prompt(
		&mut self,
		config: &BuildConfig,
		theme: &dialoguer::theme::ColorfulTheme,
	) -> Result<(), Box<dyn Error>> {
		let playbook_path: String = dialoguer::Input::with_theme(theme)
			.with_prompt("Enter the playbook path relative to the current directory")
			.interact()?;

		if !Path::new(&playbook_path).exists() {
			if !dialoguer::Confirm::with_theme(theme)
				.with_prompt("The path does not exist. Add anyway?")
				.interact()?
			{
				continue;
			}
		}
		Ok(())
	}
}

/// Run a shell command provisioner.
#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct ShellProvisioner {
	pub r#type: String,

	/// The inline command to run
	pub command: String,
}

impl ShellProvisioner {
	/// Create a new shell provisioner with inline command
	pub fn inline(command: &str) -> Self {
		Self {
			r#type: String::from("shell"),
			command: command.to_string(),
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

/// Run a shell script provisioner.
#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct ScriptProvisioner {
	pub r#type: String,
	pub script: String,
}

impl ScriptProvisioner {
	pub fn run(&self, ssh: &mut SshConnection) -> Result<(), Box<dyn Error>> {
		info!("Running script provisioner");

		if ssh.upload_exec(std::fs::read(self.script.clone())?, vec![])? != 0 {
			bail!("Provisioner failed");
		}
		Ok(())
	}
}

impl Promptable for ScriptProvisioner {
	fn prompt(
		&mut self,
		config: &BuildConfig,
		theme: &dialoguer::theme::ColorfulTheme,
	) -> Result<(), Box<dyn Error>> {
		let script_path: String = dialoguer::Input::with_theme(theme)
			.with_prompt("Enter the script path relative to the current directory")
			.interact()?;

		if !Path::new(&script_path).exists() {
			if !dialoguer::Confirm::with_theme(theme)
				.with_prompt("The path does not exist. Add anyway?")
				.interact()?
			{
				continue;
			}
		}

		Ok(())
	}
}

#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct HostnameProvisioner {
	pub hostname: String,
}

impl Promptable for HostnameProvisioner {
	fn prompt(
		&mut self,
		config: &BuildConfig,
		theme: &dialoguer::theme::ColorfulTheme,
	) -> Result<(), Box<dyn Error>> {
		self.hostname = dialoguer::Input::with_theme(theme)
			.with_prompt("Enter network hostname")
			.default(config.name.clone())
			.interact()?;

		Ok(())
	}
}

#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct TimezoneProvisioner {}

impl Promptable for TimezoneProvisioner {
	fn prompt(
		&mut self,
		config: &BuildConfig,
		theme: &dialoguer::theme::ColorfulTheme,
	) -> Result<(), Box<dyn Error>> {
		todo!()
	}
}

#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct RootPasswordContainer {
	#[validate(length(max = 64))]
	pub root_password: String,
}

impl Promptable for RootPasswordContainer {
	fn prompt(
		&mut self,
		config: &BuildConfig,
		theme: &dialoguer::theme::ColorfulTheme,
	) -> Result<(), Box<dyn Error>> {
		let root_password = dialoguer::Password::with_theme(theme)
			.with_prompt("Root password")
			.interact()?;

		todo!()
	}
}

impl Default for RootPasswordContainer {
	fn default() -> RootPasswordContainer {
		RootPasswordContainer {
			root_password: crate::random_password(),
		}
	}
}

/// Configuration for a LUKS encrypted root filesystem
#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct LuksProvisoner {
	/// The LUKS passphrase
	pub passphrase: String,

	/// Whether the LUKS passphrase will be enrolled in a TPM
	pub tpm: bool,
}

impl Promptable for LuksProvisoner {
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
		Ok(())
	}
}

/// Configuration for a Unix-like user account.
#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct UnixUserProvisioner {
	pub username: String,

	pub password: String,

	pub shell: String,
}
pub struct SshProvisioner {}

#[derive(Clone, Serialize, Deserialize, Validate, Debug, Default)]
pub struct PartitionProvisioner {
	pub storage_size: String,
}

impl PartitionProvisioner {
	pub fn storage_size_bytes(&self) -> u64 {
		self.storage_size
			.parse::<ubyte::ByteUnit>()
			.unwrap()
			.as_u64()
	}
}
