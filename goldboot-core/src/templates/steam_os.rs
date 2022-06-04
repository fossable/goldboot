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
pub enum SteamOsVersion {
	Brewmaster2_195,
}

#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct SteamOsTemplate {
	pub version: SteamOsVersion,

	#[serde(flatten)]
	pub iso: IsoContainer,

	#[serde(flatten)]
	pub general: GeneralContainer,

	pub root_password: String,

	#[serde(flatten)]
	pub provisioners: ProvisionersContainer,
}

impl Default for SteamOsTemplate {
	fn default() -> Self {
		Self {
			iso: IsoContainer {
				url: String::from(
					"https://repo.steampowered.com/download/brewmaster/2.195/SteamOSDVD.iso",
				),
				checksum: String::from("sha512:0ce55048d2c5e8a695f309abe22303dded003c93386ad28c6daafc977b3d5b403ed94d7c38917c8c837a2b1fe560184cf3cc12b9f2c4069fd70ed0deab47eb7c"),
			},
			root_password: String::from("root"),
			version: SteamOsVersion::Brewmaster2_195,
			general: GeneralContainer{
				base: TemplateBase::SteamOs,
				storage_size: String::from("15 GiB"),
				.. Default::default()
			},
			provisioners: ProvisionersContainer::default(),
		}
	}
}

impl Template for SteamOsTemplate {
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
			// Wait for bootloader
			wait_screen!("28fe084e08242584908114a5d21960fdf072adf9"),
			// Start automated install
			enter!(),
			// Wait for completion
			wait_screen!(""),
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
