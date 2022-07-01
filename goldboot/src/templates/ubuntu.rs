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

#[derive(Clone, Serialize, Deserialize, Debug)]
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

	pub edition: UbuntuEdition,

	pub release: UbuntuRelease,

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
			edition: UbuntuEdition::Desktop,
			release: UbuntuRelease::Jammy,
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
}

impl Promptable for UbuntuTemplate {
	fn prompt(
		config: &BuildConfig,
		theme: &dialoguer::theme::ColorfulTheme,
	) -> Result<serde_json::Value, Box<dyn Error>>
	where
		Self: Sized,
	{
		let mut template = UbuntuTemplate::default();

		// Prompt edition
		{
			let edition_index = dialoguer::Select::with_theme(theme)
				.with_prompt("Choose Ubuntu edition")
				.default(0)
				.item("Desktop")
				.item("Server")
				.interact()?;

			template.edition = match edition_index {
				0 => UbuntuEdition::Desktop,
				1 => UbuntuEdition::Server,
				_ => panic!(),
			};
		}

		// Prompt release
		{
			let release_index = dialoguer::Select::with_theme(theme)
				.with_prompt("Choose Ubuntu release")
				.default(0)
				.item("22.04 LTS")
				.interact()?;

			template.release = match release_index {
				0 => UbuntuRelease::Jammy,
				_ => panic!(),
			};
		}

		Ok(serde_json::to_value(template)?)
	}
}
