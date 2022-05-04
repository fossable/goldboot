use crate::{build::BuildWorker, cache::*, qemu::QemuArgs, templates::*};
use serde::{Deserialize, Serialize};
use std::error::Error;
use validator::Validate;

const DEFAULT_MIRROR: &str = "https://dl-cdn.alpinelinux.org/alpine";

/// Template for Alpine Linux images (https://www.alpinelinux.org).
#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct AlpineTemplate {
	/// The root account password
	pub root_password: String,

	#[serde(flatten)]
	pub iso: IsoContainer,

	#[serde(flatten)]
	pub general: GeneralContainer,

	#[serde(flatten)]
	pub provisioners: ProvisionersContainer,
}

impl Default for AlpineTemplate {
	fn default() -> Self {
		Self {
			root_password: String::from("root"),
			iso: IsoContainer {
				url: format!(
					"{DEFAULT_MIRROR}/v3.15/releases/x86_64/alpine-standard-3.15.0-x86_64.iso"
				),
				checksum: String::from("none"),
			},
			general: GeneralContainer {
				r#type: TemplateType::Alpine,
				storage_size: String::from("5 GiB"),
				partitions: None,
				qemuargs: None,
			},
			provisioners: ProvisionersContainer::default(),
		}
	}
}

impl Template for AlpineTemplate {
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
			// Initial wait
			wait!(60),
			// Root login
			enter!("root"),
			// Start quick install
			enter!("KEYMAPOPTS='us us' setup-alpine -q"),
		])?;

		// Wait for SSH
		let ssh = qemu.ssh_wait(context.ssh_port, "root", &self.root_password)?;

		// Run provisioners
		for provisioner in &self.provisioners.provisioners {
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
