use crate::{
	build::BuildWorker,
	cache::{MediaCache, MediaFormat},
	qemu::QemuArgs,
	templates::*,
};
use serde::{Deserialize, Serialize};
use std::error::Error;
use validator::Validate;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum DebianVersion {
	Bullseye,
	Bookworm,
	Trixie,
}

#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct DebianTemplate {
	pub root_password: String,

	#[serde(flatten)]
	pub iso: IsoContainer,

	#[serde(flatten)]
	pub general: GeneralContainer,

	pub version: DebianVersion,

	#[serde(skip_serializing_if = "Option::is_none")]
	pub provisioners: Option<Vec<Provisioner>>,
}

impl Default for DebianTemplate {
	fn default() -> Self {
		Self {
			root_password: String::from("root"),
			iso: IsoContainer {
				url: format!("https://cdimage.debian.org/debian-cd/current/amd64/iso-cd/debian-11.3.0-amd64-netinst.iso"),
				checksum: String::from("none"),
			},
			version: DebianVersion::Bullseye,
			general: GeneralContainer{
				r#type: TemplateType::Debian,
				storage_size: String::from("15 GiB"),
				partitions: None,
				qemuargs: None,
			},
			provisioners: None,
		}
	}
}

impl Template for DebianTemplate {
	fn build(&self, context: &BuildWorker) -> Result<(), Box<dyn Error>> {
		let mut qemuargs = QemuArgs::new(&context);

		qemuargs.drive.push(format!(
			"file={},if=virtio,cache=writeback,discard=ignore,format=qcow2",
			context.image_path
		));
		qemuargs.drive.push(format!(
			"file={},media=cdrom",
			MediaCache::get(self.iso.url.clone(), &self.iso.checksum, MediaFormat::Iso)?
		));

		// Start VM
		let mut qemu = qemuargs.start_process()?;

		// Send boot command
		#[rustfmt::skip]
		qemu.vnc.boot_command(vec![
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

	fn general(&self) -> GeneralContainer {
		self.general.clone()
	}
}
