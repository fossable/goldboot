use crate::cache::MediaCache;
use crate::config::Provisioner;
use crate::qemu::QemuArgs;
use crate::{
    config::Config,
    profile::Profile,
    vnc::bootcmds::{enter, input, leftSuper, spacebar, tab, wait},
};
use serde::{Deserialize, Serialize};
use std::error::Error;
use validator::Validate;

#[derive(Clone, Serialize, Deserialize, Default, Debug)]
pub enum PopOsVersions {
    #[serde(rename = "21.10")]
    #[default]
    V21_10,
}

#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct PopOsProfile {
    pub version: PopOsVersions,

    pub username: String,

    pub password: String,

    pub root_password: String,

    pub iso_url: String,

    pub iso_checksum: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub provisioners: Option<Vec<Provisioner>>,
}

impl Default for PopOsProfile {
    fn default() -> Self {
        Self {
            version: PopOsVersions::V21_10,
            username: whoami::username(),
            password: String::from("88Password;"),
            root_password: String::from("root"),
            iso_url: String::from("https://pop-iso.sfo2.cdn.digitaloceanspaces.com/21.10/amd64/intel/7/pop-os_21.10_amd64_intel_7.iso"),
            iso_checksum: String::from("sha256:93e8d3977d9414d7f32455af4fa38ea7a71170dc9119d2d1f8e1fba24826fae2"),
            provisioners: None,
        }
    }
}

impl Profile for PopOsProfile {
    fn build(&self, config: &Config, image_path: &str) -> Result<(), Box<dyn Error>> {
        let mut qemuargs = QemuArgs::new(&config);

        qemuargs.drive.push(format!(
            "file={image_path},if=virtio,cache=writeback,discard=ignore,format=qcow2"
        ));
        qemuargs.drive.push(format!(
            "file={},media=cdrom",
            MediaCache::get(self.iso_url.clone(), &self.iso_checksum)?
        ));

        // Start VM
        let mut qemu = qemuargs.start_process()?;

        // Send boot command
        qemu.vnc.boot_command(vec![
            wait!(120), // Wait for boot
            enter!(),   // Select language: English
            enter!(),   // Select location: United States
            enter!(),
            enter!(), // Select keyboard layout: US
            spacebar!(),
            enter!(), // Select clean install
            spacebar!(),
            enter!(),              // Select disk
            enter!(self.username), // Configure username
            input!(self.password),
            tab!(),
            enter!(self.password), // Configure password
            enter!(),              // Enable disk encryption
            wait!(250),
            spacebar!(),
            wait!(250),            // Wait for installation (avoiding screen timeouts)
            enter!(),              // Reboot
            wait!(30),             // Wait for reboot
            enter!(self.password), // Unlock disk
            wait!(30),             // Wait for login prompt
            enter!(),
            enter!(self.password), // Login
            wait!(60),             // Wait for login
            leftSuper!(),
            enter!("terminal"), // Open terminal
            enter!("sudo su -"),
            enter!(self.password), // Root login
            enter!("passwd"),
            enter!(self.root_password),
            enter!(self.root_password), // Change root password
            enter!("apt update"),
            wait!(30), // Update package cache
            enter!("apt install -y openssh-server"),
            wait!(30),                                                   // Install sshd
            enter!("echo 'PermitRootLogin yes' >>/etc/ssh/sshd_config"), // Configure sshd
            enter!("systemctl restart sshd"),                            // Start sshd
        ])?;

        // Wait for SSH
        let ssh = qemu.ssh()?;

        // Run provisioners
        for provisioner in &self.provisioners {
            // TODO
        }

        // Shutdown
        qemu.shutdown("poweroff")?;
        Ok(())
    }
}
