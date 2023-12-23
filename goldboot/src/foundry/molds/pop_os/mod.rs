use crate::{
    build::BuildWorker,
    cache::{MediaCache, MediaFormat},
    provisioners::*,
    qemu::QemuArgs,
    templates::*,
};
use serde::{Deserialize, Serialize};
use std::error::Error;
use validator::Validate;

#[derive(Clone, Serialize, Deserialize, Default, Debug)]
pub enum PopOsEdition {
    #[default]
    Amd,
    Nvidia,
}

#[derive(Clone, Serialize, Deserialize, Default, Debug)]
pub enum PopOsRelease {
    #[serde(rename = "21.10")]
    #[default]
    V21_10,

    #[serde(rename = "22.04")]
    V22_04,
}

#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct PopOsTemplate {
    pub id: TemplateId,
    pub edition: PopOsEdition,
    pub release: PopOsRelease,

    pub iso: IsoSource,
    pub hostname: HostnameProvisioner,

    pub username: String,

    pub password: String,

    pub root_password: String,

    pub ansible: Option<Vec<AnsibleProvisioner>>,
}

impl Default for PopOsTemplate {
    fn default() -> Self {
        Self {
			id: TemplateId::PopOs,
			edition: PopOsEdition::Amd,
            release: PopOsRelease::V21_10,
            username: whoami::username(),
            password: String::from("88Password;"),
            root_password: String::from("root"),
            iso: IsoContainer {
	            url: String::from("https://pop-iso.sfo2.cdn.digitaloceanspaces.com/21.10/amd64/intel/7/pop-os_21.10_amd64_intel_7.iso"),
	            checksum: String::from("sha256:93e8d3977d9414d7f32455af4fa38ea7a71170dc9119d2d1f8e1fba24826fae2"),
	        },
            general: GeneralContainer{
				base: TemplateBase::PopOs,
				storage_size: String::from("15 GiB"),
				.. Default::default()
			},
			provisioners: ProvisionersContainer::default(),
        }
    }
}

impl Template for PopOsTemplate {
    fn build(&self, context: &BuildWorker) -> Result<()> {
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
        qemu.vnc.boot_command(vec![
            // Wait for boot
            wait!(120),
            // Select language: English
            enter!(),
            // Select location: United States
            enter!(),
            // Select keyboard layout: US
            enter!(),
            enter!(),
            // Select clean install
            spacebar!(),
            enter!(),
            // Select disk
            spacebar!(),
            enter!(),
            // Configure username
            enter!(self.username),
            // Configure password
            input!(self.password),
            tab!(),
            enter!(self.password),
            // Enable disk encryption
            enter!(),
            // Wait for installation (avoiding screen timeouts)
            wait!(250),
            spacebar!(),
            wait!(250),
            // Reboot
            enter!(),
            wait!(30),
            // Unlock disk
            enter!(self.password),
            wait!(30),
            // Login
            enter!(),
            enter!(self.password),
            wait!(60),
            // Open terminal
            leftSuper!(),
            enter!("terminal"),
            // Root login
            enter!("sudo su -"),
            enter!(self.password),
            // Change root password
            enter!("passwd"),
            enter!(self.root_password),
            enter!(self.root_password),
            // Update package cache
            enter!("apt update"),
            wait!(30),
            // Install sshd
            enter!("apt install -y openssh-server"),
            wait!(30),
            // Configure sshd
            enter!("echo 'PermitRootLogin yes' >>/etc/ssh/sshd_config"),
            // Start sshd
            enter!("systemctl restart sshd"),
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
