use crate::{build::BuildWorker, cache::*, provisioners::*, qemu::QemuArgs, templates::*};
use serde::{Deserialize, Serialize};
use std::error::Error;
use validator::Validate;
use strum::{Display, EnumIter, IntoEnumIterator};

const DEFAULT_MIRROR: &str = "https://dl-cdn.alpinelinux.org/alpine";

#[derive(Clone, Serialize, Deserialize, Debug, EnumIter, Display)]
pub enum AlpineEdition {
	Standard,
	Extended,
	RaspberryPi,
	Xen,
}

#[derive(Clone, Serialize, Deserialize, Debug, EnumIter)]
pub enum AlpineRelease {
	Edge,
	#[serde(rename = "v3.16")]
	V3_16,
	#[serde(rename = "v3.15")]
	V3_15,
}

impl Display for AlpineRelease {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(
			f,
			"{}",
			match &self {
				AlpineRelease::Edge => "Edge",
				AlpineRelease::V3_16 => "v3.16",
				AlpineRelease::V3_15 => "v3.15",
			}
		)
	}
}

/// Template for Alpine Linux images (https://www.alpinelinux.org).
#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct AlpineTemplate {
	pub id: TemplateId,
	pub edition: AlpineEdition,
	pub release: AlpineRelease,

	pub iso: IsoProvisioner,
	pub hostname: HostnameProvisioner,
	pub users: Option<Vec<UnixAccountProvisioner>>,
	pub partitions: PartitionProvisioner,
	pub ansible: Option<Vec<AnsibleProvisioner>>,
}

impl Default for AlpineTemplate {
	fn default() -> Self {
		Self {
			id: TemplateId::Alpine,
			edition: AlpineEdition::Standard,
			release: AlpineRelease::V3_16,
			iso: IsoProvisioner {
				url: format!(
					"{DEFAULT_MIRROR}/v3.15/releases/x86_64/alpine-standard-3.15.0-x86_64.iso"
				),
				checksum: String::from("none"),
			},
			hostname: HostnameProvisioner::default(),
			users: None,
			partitions: PartitionProvisioner {
				total_size: String::from("5 GiB"),
			},
			ansible: None,
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
			wait!(30),
			// Root login
			enter!("root"),
			// Configure install
			enter!("export KEYMAPOPTS='us us'"),
			enter!("export HOSTNAMEOPTS='-n goldboot'"),
			enter!("export INTERFACESOPTS='
auto lo
iface lo inet loopback

auto eth0
iface eth0 inet dhcp
    hostname alpine-test'"
			),
			enter!("export DNSOPTS='1.1.1.1'"),
			enter!("export TIMEZONEOPTS='-z UTC'"),
			enter!("export PROXYOPTS='none'"),
			enter!("export APKREPOSOPTS='-r'"),
			enter!("export SSHDOPTS='-c openssh'"),
			enter!("export NTPOPTS='-c openntpd'"),
			enter!("export DISKOPTS='-m sys /dev/vda'"),
			// Start install
			enter!("echo -e 'root\nroot\ny' | setup-alpine"),
			wait_screen_rect!("6d7b9fc9229c4f4ae8bc84f0925d8479ccd3e7d2", 668, 0, 1024, 100),
			// Remount root partition
			enter!("mount -t ext4 /dev/vda3 /mnt"),
			// Configure SSH
			enter!("echo 'PermitRootLogin yes' >>/mnt/etc/ssh/sshd_config"),
			// Reboot into installation
			enter!("apk add efibootmgr; efibootmgr -n 0003; reboot"),
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
}

impl Promptable for AlpineTemplate {
	fn prompt(
		&mut self,
		config: &BuildConfig,
		theme: &dialoguer::theme::ColorfulTheme,
	) -> Result<(), Box<dyn Error>> {
		// Prompt edition
		{
			let editions: Vec<AlpineEdition> = AlpineEdition::iter().collect();
			let edition_index = dialoguer::Select::with_theme(theme)
				.with_prompt("Choose an edition")
				.default(0)
				.items(&editions)
				.interact()?;

			self.edition = editions[edition_index];
		}

		// Prompt mirror list
		// TODO

		self.validate()?;
		Ok(())
	}
}
