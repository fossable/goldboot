use crate::{
	build::BuildWorker,
	cache::{MediaCache, MediaFormat},
	http::HttpServer,
	qemu::QemuArgs,
	templates::{debian::*, *},
};
use colored::*;
use log::info;
use serde::{Deserialize, Serialize};
use simple_error::bail;
use std::error::Error;
use validator::Validate;

#[derive(rust_embed::RustEmbed)]
#[folder = "res/GoldbootLinux/"]
struct Resources;

#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct GoldbootLinuxTemplate {
	#[serde(flatten)]
	pub general: GeneralContainer,

	/// The goldboot-linux executable to embed
	pub executable: String,
}

impl Default for GoldbootLinuxTemplate {
	fn default() -> Self {
		Self {
			general: GeneralContainer {
				base: TemplateBase::GoldbootLinux,
				storage_size: String::from("4 GiB"),
				..Default::default()
			},
			executable: String::from(""),
		}
	}
}

impl Template for GoldbootLinuxTemplate {
	fn build(&self, context: &BuildWorker) -> Result<(), Box<dyn Error>> {
		info!("Starting {} build", "Goldboot Linux".blue());

		let mut qemuargs = QemuArgs::new(&context);

		qemuargs.drive.push(format!(
			"file={},if=virtio,cache=writeback,discard=ignore,format=qcow2",
			context.image_path
		));
		qemuargs.drive.push(format!(
			"file={},media=cdrom",
			MediaCache::get("https://cdimage.debian.org/cdimage/weekly-builds/amd64/iso-cd/debian-testing-amd64-netinst.iso".to_string(), "none", MediaFormat::Iso)?
		));

		// Start HTTP
		let http = HttpServer::serve_file(Resources::get("preseed.cfg").unwrap().data.to_vec())?;

		// Start VM
		let mut qemu = qemuargs.start_process()?;

		// Temporary root password for the run
		let temp_password = crate::random_password();

		// Send boot command
		#[rustfmt::skip]
		qemu.vnc.boot_command(vec![
			wait!(10),
			input!("aa"),
			wait_screen!("a5263becea998337f06070678e4bf3db2d437195"),
			enter!(format!("http://10.0.2.2:{}/preseed.cfg", http.port)),
			wait_screen!("97354165fd270a95fd3da41ef43c35bf24b7c09b"),
			enter!(&temp_password),
			enter!(&temp_password),
			wait_screen!("33e3bacbff9507e9eb29c73642eaceda12a359c2"),
		])?;

		// Wait for SSH
		let mut ssh = qemu.ssh_wait(context.ssh_port, "root", &temp_password)?;

		// Copy executable
		ssh.upload(
			std::fs::read(&self.executable)?,
			"/mnt/usr/bin/goldboot-linux",
		)?;

		// Shutdown
		ssh.shutdown("poweroff")?;
		qemu.shutdown_wait()?;
		Ok(())
	}

	fn general(&self) -> GeneralContainer {
		self.general.clone()
	}
}
