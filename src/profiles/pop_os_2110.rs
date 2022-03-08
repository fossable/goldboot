use crate::packer::bootcmds::{enter, input, leftSuper, spacebar, tab, wait};
use crate::packer::QemuBuilder;
use crate::Config;
use anyhow::{bail, Result};
use std::path::Path;

pub fn init(config: &mut Config) {
    config.base = Some(String::from("PopOs2110"));
    config.profile.insert("username".into(), "user".into());
    config
        .profile
        .insert("password".into(), "88Password**".into());
    config.profile.insert("root_password".into(), "root".into());
    config.iso_url = "https://pop-iso.sfo2.cdn.digitaloceanspaces.com/21.10/amd64/intel/7/pop-os_21.10_amd64_intel_7.iso";
    config.iso_checksum = Some("sha256:93e8d3977d9414d7f32455af4fa38ea7a71170dc9119d2d1f8e1fba24826fae2");
}

pub fn validate(config: &Config) -> Result<()> {
    // We can't use the root account for initial setup
    match config.profile.get("username") {
        Some(username) => {
            if username == "root" {
                bail!("Cannot use the root account for initial setup");
            }
        }
        None => bail!("No user given"),
    }
    Ok(())
}

pub fn build(config: &Config, _context: &Path) -> Result<QemuBuilder> {
    validate(&config)?;

    let username = config.profile.get("username").unwrap();
    let password = config.profile.get("password").unwrap();
    let root_password = config.profile.get("root_password").unwrap();

    let mut builder = QemuBuilder::new();
    builder.boot_command = vec![
        enter!(), // Select language: English
        enter!(), // Select location: United States
        enter!(),
        enter!(), // Select keyboard layout: US
        spacebar!(),
        enter!(), // Select clean install
        spacebar!(),
        enter!(),         // Select disk
        enter!(username), // Configure username
        input!(password),
        tab!(),
        enter!(password), // Configure password
        enter!(),         // Enable disk encryption
        wait!(250), spacebar!(), wait!(250), // Wait for installation (avoiding screen timeouts)
        enter!(),         // Reboot
        wait!(30),        // Wait for reboot
        enter!(password), // Unlock disk
        wait!(30),        // Wait for login prompt
        enter!(),
        enter!(password), // Login
        wait!(60),        // Wait for login
        leftSuper!(),
        enter!("terminal"), // Open terminal
        enter!("sudo su -"),
        enter!(password), // Root login
        enter!("passwd"),
        enter!(root_password),
        enter!(root_password), // Change root password
        enter!("apt update"),
        wait!(30), // Update package cache
        enter!("apt install -y openssh-server"),
        wait!(30),                                                   // Install sshd
        enter!("echo 'PermitRootLogin yes' >>/etc/ssh/sshd_config"), // Configure sshd
        enter!("systemctl restart sshd"),                            // Start sshd
    ];

    builder.boot_wait = "2m";
    builder.communicator = "ssh".into();
    builder.shutdown_command = "poweroff".into();
    builder.ssh_password = Some("root".into());
    builder.ssh_username = Some(root_password.to_string());
    builder.ssh_wait_timeout = Some("5m".into());

    return Ok(builder);
}
