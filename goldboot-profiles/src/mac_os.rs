use crate::config::Provisioner;
use crate::qemu::QemuArgs;
use crate::{
    config::Config,
    profile::Profile,
    vnc::bootcmds::{enter, tab, wait_screen_rect},
};
use serde::{Deserialize, Serialize};
use std::error::Error;
use validator::Validate;

#[derive(rust_embed::RustEmbed)]
#[folder = "res/mac_os/"]
struct Resources;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum MacOsVersion {
    Catalina,
    BigSur,
    Monterey,
}

#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct MacOsProfile {
    pub version: MacOsVersion,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub provisioners: Option<Vec<Provisioner>>,
}

impl Default for MacOsProfile {
    fn default() -> Self {
        Self {
            provisioners: None,
            version: MacOsVersion::Monterey,
        }
    }
}

impl Profile for MacOsProfile {
    fn build(&self, config: &Config, image_path: &str) -> Result<(), Box<dyn Error>> {
        let mut qemuargs = QemuArgs::new(&config);

        // Acquire temporary directory for this one
        let tmp = tempfile::tempdir()?;

        // Copy OpenCore partition
        if let Some(resource) = Resources::get("OpenCore.qcow2") {
            std::fs::write(tmp.path().join("OpenCore.qcow2"), resource.data)?;
        }

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
            tmp.path().to_string_lossy()
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
            "file={image_path},id=System,if=none,cache=writeback,discard=ignore,format=qcow2"
        ));
        qemuargs
            .device
            .push(format!("ide-hd,bus=sata.4,drive=System"));

        // Start VM
        let mut qemu = qemuargs.start_process()?;

        // Send boot command
        match self.version {
            MacOsVersion::Monterey => {
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
        }

        // Wait for SSH
        let ssh = qemu.ssh_wait(config.ssh_port.unwrap(), "root", "root")?;

        // Run provisioners
        for provisioner in &self.provisioners {
            // TODO
        }

        // Shutdown
        ssh.shutdown("shutdown -h now")?;
        qemu.shutdown_wait()?;
        Ok(())
    }
}
