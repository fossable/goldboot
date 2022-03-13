use crate::{
    config::Config,
    packer::bootcmds::{enter, input, leftSuper, spacebar, tab, wait},
    packer::{PackerTemplate, QemuBuilder},
    profile::Profile,
};
use serde::{Deserialize, Serialize};
use std::{error::Error, path::Path};
use validator::Validate;

#[derive(Clone, Serialize, Deserialize, Default)]
enum PopOsVersions {
    #[serde(rename = "21.10")]
    #[default]
    V21_10,
}

#[derive(Clone, Serialize, Deserialize, Validate, Default)]
pub struct PopOsProfile {
    pub version: PopOsVersions,

    #[serde(default = "default_username")]
    username: String,

    #[serde(default = "default_password")]
    password: String,

    #[serde(default = "default_root_password")]
    root_password: String,

    #[serde(default = "default_iso_url")]
    iso_url: String,

    #[serde(default = "default_iso_checksum")]
    iso_checksum: String,
}

fn default_username() -> String {
    whoami::username()
}

fn default_password() -> String {
    String::from("88Password;")
}

fn default_root_password() -> String {
    String::from("root")
}

fn default_iso_url() -> String {
    String::from("https://pop-iso.sfo2.cdn.digitaloceanspaces.com/21.10/amd64/intel/7/pop-os_21.10_amd64_intel_7.iso")
}

fn default_iso_checksum() -> String {
    String::from("sha256:93e8d3977d9414d7f32455af4fa38ea7a71170dc9119d2d1f8e1fba24826fae2")
}

impl Profile for PopOsProfile {
    fn generate_template(&self, context: &Path) -> Result<PackerTemplate, Box<dyn Error>> {
        let mut template = PackerTemplate::default();

        let mut builder = QemuBuilder::new();
        builder.boot_command = vec![
            enter!(), // Select language: English
            enter!(), // Select location: United States
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
        ];

        builder.boot_wait = String::from("2m");
        builder.communicator = String::from("ssh");
        builder.shutdown_command = String::from("poweroff");
        builder.ssh_password = Some(String::from("root"));
        builder.ssh_username = Some(self.root_password.clone());
        builder.ssh_wait_timeout = Some(String::from("5m"));
        template.builders.push(builder);

        Ok(template)
    }
}
