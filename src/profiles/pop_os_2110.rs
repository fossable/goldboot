use crate::{
    config::Config,
    packer::bootcmds::{enter, input, leftSuper, spacebar, tab, wait},
    packer::PackerTemplate,
    profile::Profile,
};
use std::{error::Error, path::Path};
use validator::Validate;

#[derive(Validate)]
struct PopOs2110Profile {
    username: String,
    password: String,
    root_password: String,
}

pub fn init(config: &mut Config) {
    config.base = Some(String::from("PopOs2110"));
    config
        .profile
        .insert(String::from("username"), String::from("user"));
    config
        .profile
        .insert(String::from("password"), String::from("88Password**"));
    config
        .profile
        .insert(String::from("root_password"), String::from("root"));
    config.iso_url = String::from("https://pop-iso.sfo2.cdn.digitaloceanspaces.com/21.10/amd64/intel/7/pop-os_21.10_amd64_intel_7.iso");
    config.iso_checksum = Some(String::from(
        "sha256:93e8d3977d9414d7f32455af4fa38ea7a71170dc9119d2d1f8e1fba24826fae2",
    ));
}

impl PopOs2110Profile {
    fn new(config: &Config) -> Result<Self, Box<dyn Error>> {
        let profile = Self {
            username: config
                .profile
                .get("username")
                .ok_or("Missing username")?
                .to_string(),
            password: config
                .profile
                .get("password")
                .ok_or("Missing password")?
                .to_string(),
            root_password: config
                .profile
                .get("root_password")
                .ok_or("Missing root_password")?
                .to_string(),
        };

        // TODO prohibit root username
        profile.validate()?;
        Ok(profile)
    }
}

impl Profile for PopOs2110Profile {
    fn build(&self, template: &mut PackerTemplate, context: &Path) -> Result<(), Box<dyn Error>> {
        let mut builder = template.builders.first().unwrap();
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
        builder.ssh_username = Some(self.root_password);
        builder.ssh_wait_timeout = Some(String::from("5m"));
        Ok(())
    }
}
