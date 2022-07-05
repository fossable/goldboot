use crate::{build::BuildWorker, cache::MediaCache, qemu::QemuArgs, templates::*};
use serde::{Deserialize, Serialize};
use std::error::Error;
use validator::Validate;

//#[derive(rust_embed::RustEmbed)]
//#[folder = "res/MacOs/"]
//struct Resources;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum MacOsRelease {
	Catalina,
	BigSur,
	Monterey,
}

#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct MacOsTemplate {
	pub id: TemplateId,
	pub release: MacOsRelease,

	#[serde(flatten)]
	pub general: GeneralContainer,

	#[serde(flatten)]
	pub provisioners: ProvisionersContainer,
}

impl Default for MacOsTemplate {
	fn default() -> Self {
		Self {
			id: TemplateId::MacOs,
			release: MacOsRelease::Monterey,
			provisioners: ProvisionersContainer {
				provisioners: Some(vec![
					serde_json::to_value(ShellProvisioner::inline("/bin/bash -c \"$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)\"")).unwrap(),
				])
			},
			version: MacOsVersion::Monterey,
			general: GeneralContainer{
				base: TemplateBase::MacOs,
				storage_size: String::from("50 GiB"),
				.. Default::default()
			},
		}
	}
}

impl Template for MacOsTemplate {
	fn build(&self, context: &BuildWorker) -> Result<(), Box<dyn Error>> {
		let mut qemuargs = QemuArgs::new(&context);

		// Copy OpenCore partition
		//if let Some(resource) = Resources::get("OpenCore.qcow2") {
		//	std::fs::write(context.tmp.path().join("OpenCore.qcow2"), resource.data)?;
		//}

		// Convert dmg to img
		//qemu-img convert BaseSystem.dmg -O raw BaseSystem.img

		qemuargs.cpu = Some(format!("Penryn,kvm=on,vendor=GenuineIntel,+invtsc,vmware-cpuid-freq=on,+ssse3,+sse4.2,+popcnt,+avx,+aes,+xsave,+xsaveopt,check"));
		qemuargs.machine = format!("q35,accel=kvm");
		qemuargs.smbios = Some(format!("type=2"));
		qemuargs.device.push(format!("ich9-ahci,id=sata"));
		qemuargs.device.push(format!("usb-ehci,id=ehci"));
		qemuargs.device.push(format!("nec-usb-xhci,id=xhci"));
		qemuargs.device.push(format!(
			"isa-applesmc,osk=ourhardworkbythesewordsguardedpleasedontsteal(c)AppleComputerInc"
		));
		qemuargs.usbdevice.push(format!("keyboard"));
		qemuargs.usbdevice.push(format!("tablet"));
		qemuargs.global.push(format!("nec-usb-xhci.msi=off"));

		// Add boot partition
		qemuargs.drive.push(format!(
			"file={}/OpenCore.qcow2,id=OpenCore,if=none,format=qcow2",
			context.tmp.path().to_string_lossy()
		));
		qemuargs
			.device
			.push(format!("ide-hd,bus=sata.2,drive=OpenCore"));

		// Add install media
		qemuargs.drive.push(format!(
			"file=/home/cilki/OSX-KVM/BaseSystem.img,id=InstallMedia,if=none,format=raw"
		));
		qemuargs
			.device
			.push(format!("ide-hd,bus=sata.3,drive=InstallMedia"));

		// Add system drive
		qemuargs.drive.push(format!(
			"file={},id=System,if=none,cache=writeback,discard=ignore,format=qcow2",
			context.image_path,
		));
		qemuargs
			.device
			.push(format!("ide-hd,bus=sata.4,drive=System"));

		// Start VM
		let mut qemu = qemuargs.start_process()?;

		// Send boot command
		match self.release {
			MacOsRelease::Monterey => {
				#[rustfmt::skip]
				qemu.vnc.boot_command(vec![
					enter!(),
					enter!("diskutil eraseDisk APFS System disk0"),
					// Wait for "Select your region" screen
					wait_screen_rect!("fa1aeec4a3d4436d9bdd99345b29256ce4d141c8", 50, 0, 1024, 700),
					// Configure region
					enter!("United States"), tab!(), tab!(), enter!(),
					// ...
					// Configure ssh
					enter!("echo 'PermitRootLogin yes' >>/etc/ssh/sshd_config"),
					// Start sshd
					enter!("launchctl load -w /System/Library/LaunchDaemons/ssh.plist"),
				])?;
			}
			_ => {}
		}

		// Wait for SSH
		let mut ssh = qemu.ssh_wait(context.ssh_port, "root", "root")?;

		// Run provisioners
		self.provisioners.run(&mut ssh)?;

		// Shutdown
		ssh.shutdown("shutdown -h now")?;
		qemu.shutdown_wait()?;
		Ok(())
	}
}
