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
pub enum UbuntuRelease {
	Jammy,
}

pub enum UbuntuEdition {
	Server,
	Desktop,
}

#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct UbuntuTemplate {
	pub root_password: String,

	/// The installation media
	pub iso: IsoContainer,

	#[serde(flatten)]
	pub general: GeneralContainer,

	pub version: UbuntuRelease,

	#[serde(flatten)]
	pub provisioners: ProvisionersContainer,
}

impl Default for UbuntuTemplate {
	fn default() -> Self {
		Self {
			root_password: String::from("root"),
			iso: IsoContainer {
				url: format!(""),
				checksum: String::from("none"),
			},
			version: UbuntuRelease::Jammy,
			general: GeneralContainer {
				base: TemplateBase::Ubuntu,
				storage_size: String::from("15 GiB"),
				..Default::default()
			},
			provisioners: ProvisionersContainer::default(),
		}
	}
}

impl Template for UbuntuTemplate {
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
		let mut ssh = qemu.ssh_wait(context.ssh_port, "root", &self.root_password)?;

		// Run provisioners
		self.provisioners.run(&mut ssh)?;

		// Shutdown
		ssh.shutdown("poweroff")?;
		qemu.shutdown_wait()?;
		Ok(())
	}

	fn general(&self) -> GeneralContainer {
		self.general.clone()
	}

	fn prompt(
		config: &BuildConfig,
		theme: &dialoguer::theme::ColorfulTheme,
	) -> Result<serde_json::Value, Box<dyn Error>>
	where
		Self: Sized,
	{

		// Prompt edition
	}
}
