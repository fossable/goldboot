use crate::ssh::SshConnection;
use log::{debug, info};
use rand::Rng;
use serde::{Deserialize, Serialize};
use simple_error::bail;
use std::{default::Default, error::Error, fs, fs::File, path::Path, process::Command};
use validator::Validate;

pub mod cache;
pub mod qemu;
pub mod ssh;
pub mod templates;
pub mod vnc;
pub mod windows;

#[derive(rust_embed::RustEmbed)]
#[folder = "res/"]
struct Resources;

/// Represents a local goldboot image.
#[derive(Clone, Serialize, Deserialize, Validate)]
pub struct ImageMetadata {
	pub sha256: String,

	/// The file size in bytes
	pub size: u64,

	pub last_modified: u64,

	pub config: Config,
}

/// Search filesystem for UEFI firmware.
pub fn find_ovmf() -> Option<String> {
	for path in vec![
		"/usr/share/ovmf/x64/OVMF.fd",
		"/usr/share/OVMF/OVMF_CODE.fd",
	] {
		if Path::new(&path).is_file() {
			debug!("Located OVMF firmware at: {}", path.to_string());
			return Some(path.to_string());
		}
	}

	debug!("Failed to locate existing OVMF firmware");
	None
}

pub fn compact_image(path: &str) -> Result<(), Box<dyn Error>> {
	let tmp_path = format!("{}.out", &path);

	info!("Compacting image");
	if let Some(code) = Command::new("qemu-img")
		.arg("convert")
		.arg("-c")
		.arg("-O")
		.arg("qcow2")
		.arg(&path)
		.arg(&tmp_path)
		.stdout(std::process::Stdio::null())
		.stderr(std::process::Stdio::null())
		.status()
		.expect("Failed to launch qemu-img")
		.code()
	{
		if code != 0 {
			bail!("qemu-img failed with error code: {}", code);
		}

		let before = std::fs::metadata(&path)?.len();
		let after = std::fs::metadata(&tmp_path)?.len();

		if after < before {
			info!("Reduced image size from {} to {}", before, after);

			// Replace the original before returning
			std::fs::rename(&tmp_path, &path)?;
		} else {
			std::fs::remove_file(&tmp_path)?;
		}
	} else {
		debug!("Failed to launch qemu-img, skipping image compaction");
	}
	Ok(())
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

	#[serde(skip_serializing_if = "Option::is_none")]
	pub arch: Option<String>,

	/// The amount of memory to allocate to the VM
	pub memory: String,

	/// The size of the disk to attach to the VM
	pub disk_size: String,

	#[serde(skip_serializing_if = "Option::is_none")]
	pub nvme: Option<bool>,

	pub qemuargs: Vec<Vec<String>>,

	pub template: Option<serde_json::Value>,

	pub templates: Option<Vec<serde_json::Value>>,
}

impl BuildConfig {
	pub fn disk_size_bytes(&self) -> u64 {
		self.disk_size.parse::<ubyte::ByteUnit>().unwrap().as_u64()
	}
}

#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct Partition {
	pub r#type: String,
	pub size: String,
	pub label: String,
	pub format: String,
}

/// A generic provisioner
#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct Provisioner {
	pub r#type: String,

	#[serde(flatten)]
	pub ansible: AnsibleProvisioner,

	#[serde(flatten)]
	pub shell: ShellProvisioner,
}

impl Provisioner {
	pub fn run(&self, ssh: &SshConnection) -> Result<(), Box<dyn Error>> {
		// Check for inline command
		if let Some(command) = &self.shell.inline {
			if ssh.exec(command)? != 0 {
				bail!("Provisioning failed");
			}
		}

		// Check for shell scripts to upload
		for script in &self.shell.scripts {
			ssh.upload(&mut File::open(script)?, ".gb_script")?;

			// Execute it
			ssh.exec(".gb_script")?;
		}

		// Run an ansible playbook
		if let Some(playbook) = &self.ansible.playbook {
			if let Some(code) = Command::new("ansible-playbook")
				.arg("-u")
				.arg(ssh.username.clone())
				.arg("-p")
				.arg(ssh.password.clone())
				.arg(&playbook)
				.status()
				.expect("Failed to launch ansible-playbook")
				.code()
			{}
		}
		Ok(())
	}
}

#[derive(Clone, Serialize, Deserialize, Validate, Debug, Default)]
pub struct AnsibleProvisioner {
	pub playbook: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Validate, Debug, Default)]
pub struct ShellProvisioner {
	pub scripts: Vec<String>,
	pub inline: Option<String>,
}

impl ShellProvisioner {
	/// Create a new shell provisioner with inline command
	pub fn inline(command: &str) -> Provisioner {
		Provisioner {
			r#type: String::from("shell"),
			ansible: AnsibleProvisioner::default(),
			shell: ShellProvisioner {
				inline: Some(command.to_string()),
				scripts: vec![],
			},
		}
	}
}

#[cfg(test)]
mod tests {
	#[test]
	fn it_works() {
		let result = 2 + 2;
		assert_eq!(result, 4);
	}
}
