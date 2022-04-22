use crate::{build::BuildContext, cache::MediaCache, qemu::QemuArgs, templates::*};
use serde::{Deserialize, Serialize};
use std::error::Error;
use validator::Validate;

#[derive(Clone, Serialize, Deserialize, Default, Debug)]
pub enum PopOsVersions {
	#[serde(rename = "21.10")]
	#[default]
	V21_10,
}

#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct PopOsTemplate {
	pub version: PopOsVersions,

	pub username: String,

	pub password: String,

	pub root_password: String,

	pub iso_url: String,

	pub iso_checksum: String,

	#[serde(skip_serializing_if = "Option::is_none")]
	pub provisioners: Option<Vec<Provisioner>>,
}

impl Default for PopOsTemplate {
	fn default() -> Self {
		Self {
            version: PopOsVersions::V21_10,
            username: whoami::username(),
            password: String::from("88Password;"),
            root_password: String::from("root"),
            iso_url: String::from("https://pop-iso.sfo2.cdn.digitaloceanspaces.com/21.10/amd64/intel/7/pop-os_21.10_amd64_intel_7.iso"),
            iso_checksum: String::from("sha256:93e8d3977d9414d7f32455af4fa38ea7a71170dc9119d2d1f8e1fba24826fae2"),
            provisioners: None,
        }
	}
}

impl Template for PopOsTemplate {
	fn build(&self, context: &BuildContext) -> Result<(), Box<dyn Error>> {
		let mut qemuargs = QemuArgs::new(&context);

		qemuargs.drive.push(format!(
			"file={},if=virtio,cache=writeback,discard=ignore,format=qcow2",
			context.image_path
		));
		qemuargs.drive.push(format!(
			"file={},media=cdrom",
			MediaCache::get(self.iso_url.clone(), &self.iso_checksum)?
		));

		// Start VM
		let mut qemu = qemuargs.start_process()?;

		// Send boot command
		qemu.vnc.boot_command(vec![
			// Wait for boot
			wait!(120),
			// Select language: English
			enter!(),
			// Select location: United States
			enter!(),
			// Select keyboard layout: US
			enter!(),
			enter!(),
			// Select clean install
			spacebar!(),
			enter!(),
			// Select disk
			spacebar!(),
			enter!(),
			// Configure username
			enter!(self.username),
			// Configure password
			input!(self.password),
			tab!(),
			enter!(self.password),
			// Enable disk encryption
			enter!(),
			// Wait for installation (avoiding screen timeouts)
			wait!(250),
			spacebar!(),
			wait!(250),
			// Reboot
			enter!(),
			wait!(30),
			// Unlock disk
			enter!(self.password),
			wait!(30),
			// Login
			enter!(),
			enter!(self.password),
			wait!(60),
			// Open terminal
			leftSuper!(),
			enter!("terminal"),
			// Root login
			enter!("sudo su -"),
			enter!(self.password),
			// Change root password
			enter!("passwd"),
			enter!(self.root_password),
			enter!(self.root_password),
			// Update package cache
			enter!("apt update"),
			wait!(30),
			// Install sshd
			enter!("apt install -y openssh-server"),
			wait!(30),
			// Configure sshd
			enter!("echo 'PermitRootLogin yes' >>/etc/ssh/sshd_config"),
			// Start sshd
			enter!("systemctl restart sshd"),
		])?;

		// Wait for SSH
		let ssh = qemu.ssh_wait(context.ssh_port, "root", &self.root_password)?;

		// Run provisioners
		for provisioner in &self.provisioners {
			// TODO
		}

		// Shutdown
		ssh.shutdown("poweroff")?;
		qemu.shutdown_wait()?;
		Ok(())
	}
}
