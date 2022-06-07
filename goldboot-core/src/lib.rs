#![feature(seek_stream_len)]

use crate::{
	ssh::SshConnection,
	templates::{Template, TemplateBase},
};
use log::{debug, info};
use rand::Rng;
use serde::{Deserialize, Serialize};
use simple_error::bail;
use std::{default::Default, error::Error, net::TcpListener, process::Command};
use validator::Validate;

pub mod build;
pub mod cache;
pub mod http;
pub mod image;
pub mod progress;
pub mod qcow;
pub mod qemu;
pub mod ssh;
pub mod templates;
pub mod vnc;
pub mod windows;
pub mod registry;

/// Find a random open TCP port in the given range.
pub fn find_open_port(lower: u16, upper: u16) -> u16 {
	let mut rand = rand::thread_rng();

	loop {
		let port = rand.gen_range(lower..upper);
		match TcpListener::bind(format!("0.0.0.0:{port}")) {
			Ok(_) => break port,
			Err(_) => continue,
		}
	}
}

/// Generate a random password
pub fn random_password() -> String {
	// TODO check for a dictionary to generate something memorable

	// Fallback to random letters and numbers
	rand::thread_rng()
		.sample_iter(&rand::distributions::Alphanumeric)
		.take(12)
		.map(char::from)
		.collect()
}

pub fn is_interactive() -> bool {
	!std::env::var("CI").is_ok()
}

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
#[serde(tag = "arch")]
#[allow(non_camel_case_types)]
pub enum Architecture {
	#[default]
	amd64,
	arm64,
	i386,
	mips,
	s390x,
}

impl TryFrom<String> for Architecture {
	type Error = Box<dyn Error>;
	fn try_from(s: String) -> Result<Self, Self::Error> {
		match s.as_str() {
			"amd64" => Ok(Architecture::amd64),
			"x86_64" => Ok(Architecture::amd64),
			"arm64" => Ok(Architecture::arm64),
			"aarch64" => Ok(Architecture::arm64),
			"i386" => Ok(Architecture::i386),
			_ => bail!("Unknown architecture"),
		}
	}
}

impl ToString for Architecture {

	fn to_string(&self) -> String {
		match self {
			Architecture::amd64 => String::from("amd64"),
			_ => todo!(),

		}
	}
}

/// The global configuration
#[derive(Clone, Serialize, Deserialize, Validate, Default, Debug)]
pub struct BuildConfig {
	/// The image name
	#[validate(length(min = 1))]
	pub name: String,

	/// An image description
	#[validate(length(max = 4096))]
	#[serde(skip_serializing_if = "Option::is_none")]
	pub description: Option<String>,

	/// The system architecture
	#[serde(flatten)]
	pub arch: Architecture,

	/// The amount of memory to allocate to the VM
	#[serde(skip_serializing_if = "Option::is_none")]
	pub memory: Option<String>,

	#[serde(skip_serializing_if = "Option::is_none")]
	pub nvme: Option<bool>,

	#[validate(length(min = 1))]
	pub templates: Vec<serde_json::Value>,
}

impl BuildConfig {
	pub fn get_templates(&self) -> Result<Vec<Box<dyn Template>>, Box<dyn Error>> {
		let mut templates: Vec<Box<dyn Template>> = Vec::new();

		for template in &self.templates {
			// Get type
			let t: TemplateBase = serde_json::from_value(template.to_owned())?;
			templates.push(t.parse_template(template.to_owned())?);
		}

		Ok(templates)
	}

	pub fn get_template_bases(&self) -> Result<Vec<String>, Box<dyn Error>> {
		let mut bases: Vec<String> = Vec::new();

		for template in &self.templates {
			// Get base
			bases.push(template.get("base").unwrap().as_str().unwrap().to_string());
		}

		Ok(bases)
	}
}

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

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_find_open_port() {
		let port = find_open_port(9000, 9999);

		assert!(port < 9999);
		assert!(port >= 9000);
	}
}
