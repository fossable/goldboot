use crate::{
	build::BuildWorker,
	cache::{MediaCache, MediaFormat},
	http::HttpServer,
	qemu::QemuArgs,
	templates::*,
};
use serde::{Deserialize, Serialize};
use std::{
	error::Error,
	io::{BufRead, BufReader},
};
use validator::Validate;

#[derive(rust_embed::RustEmbed)]
#[folder = "res/Debian/"]
struct Resources;

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub enum DebianEdition {
	#[default]
	Bullseye,
	Bookworm,
	Trixie,
	Sid,
}

/// Fetch the latest ISO
pub fn fetch_debian_iso(
	edition: DebianEdition,
	arch: Architecture,
) -> Result<IsoContainer, Box<dyn Error>> {
	let arch = match arch {
		Architecture::amd64 => "amd64",
		Architecture::arm64 => "arm64",
		Architecture::i386 => "i386",
		_ => bail!("Unsupported architecture"),
	};
	let version = match edition {
		DebianEdition::Bullseye => "11.2.0",
		_ => bail!("Unsupported edition"),
	};

	let rs = reqwest::blocking::get(format!(
		"https://cdimage.debian.org/cdimage/archive/{version}/{arch}/iso-cd/SHA256SUMS"
	))?;
	if rs.status().is_success() {
		for line in BufReader::new(rs).lines().filter_map(|result| result.ok()) {
			if line.ends_with(".iso") {
				let split: Vec<&str> = line.split_whitespace().collect();
				if let [hash, filename] = split[..] {
					return Ok(IsoContainer{
						url: format!("https://cdimage.debian.org/cdimage/archive/{version}/{arch}/iso-cd/{filename}"),
						checksum: format!("sha256:{hash}"),
					});
				}
			}
		}
	}
	bail!("Failed to request latest ISO");
}

#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct DebianTemplate {
	pub id: TemplateId,
	pub root_password: String,

	/// The installation media
	pub iso: IsoContainer,

	#[serde(flatten)]
	pub general: GeneralContainer,

	pub edition: DebianEdition,

	#[serde(flatten)]
	pub provisioners: ProvisionersContainer,
}

impl Default for DebianTemplate {
	fn default() -> Self {
		Self {
			root_password: String::from("root"),
			iso: fetch_debian_iso(DebianEdition::Bullseye, Architecture::amd64).unwrap(),
			general: GeneralContainer {
				base: TemplateBase::Debian,
				storage_size: String::from("15 GiB"),
				..Default::default()
			},
			edition: DebianEdition::default(),
			provisioners: ProvisionersContainer::default(),
		}
	}
}

impl Template for DebianTemplate {
	fn build(&self, context: &BuildWorker) -> Result<(), Box<dyn Error>> {
		let mut qemuargs = QemuArgs::new(&context);

		// Start HTTP
		let http =
			HttpServer::serve_file(Resources::get("default/preseed.cfg").unwrap().data.to_vec())?;

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
			wait!(10),
			input!("aa"),
			wait_screen!("53471d73e98f0109ce3262d9c45c522d7574366b"),
			enter!(format!("http://10.0.2.2:{}/preseed.cfg", http.port)),
			wait_screen!("97354165fd270a95fd3da41ef43c35bf24b7c09b"),
			enter!(&self.root_password),
			enter!(&self.root_password),
			wait_screen!("33e3bacbff9507e9eb29c73642eaceda12a359c2"),
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
