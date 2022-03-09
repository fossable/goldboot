use crate::{config::Config, packer::bootcmds::enter, packer::PackerTemplate, profile::Profile};
use rust_embed::RustEmbed;
use std::{error::Error, path::Path};
use validator::Validate;

#[derive(RustEmbed)]
#[folder = "res/arch_linux/"]
struct Resources;

#[derive(Validate)]
pub struct ArchLinuxProfile {
    root_password: String,
}

pub fn init(config: &mut Config) {
    config.base = Some(String::from("ArchLinux"));
    config.profile.insert("username".into(), "user".into());
    config
        .profile
        .insert("password".into(), "88Password**".into());
    config.profile.insert("root_password".into(), "root".into());
    config.iso_url = String::from(
        "https://mirrors.edge.kernel.org/archlinux/iso/latest/archlinux-2022.03.01-x86_64.iso",
    );
    config.memory = String::from("2048");
}

impl ArchLinuxProfile {
    pub fn new(config: &mut Config) -> Result<Self, Box<dyn Error>> {
        let profile = Self {
            root_password: config
                .profile
                .get("root_password")
                .ok_or("Missing root_password")?
                .to_string(),
        };

        profile.validate()?;
        Ok(profile)
    }
}

impl Profile for ArchLinuxProfile {
    fn build(&self, template: &mut PackerTemplate, context: &Path) -> Result<(), Box<dyn Error>> {
        // Create install provisioner
        // TODO

        let mut builder = template.builders.first().unwrap();
        builder.boot_command = vec![
            enter!("passwd"),
            enter!(self.root_password),
            enter!(self.root_password),     // Configure root password
            enter!("systemctl start sshd"), // Start sshd
        ];
        builder.boot_wait = String::from("50s");
        builder.communicator = "ssh".into();
        builder.shutdown_command = "poweroff".into();
        builder.ssh_password = Some("root".into());
        builder.ssh_username = Some("root".into());
        builder.ssh_wait_timeout = Some("5m".into());

        Ok(())
    }
}
