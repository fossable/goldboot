use rust_embed::RustEmbed;
use std::{
    path::Path,
    error::Error,
};
use crate::{
    packer::bootcmds::{enter},
    packer::QemuBuilder,
    config::Config,
};

#[derive(RustEmbed)]
#[folder = "res/arch_linux/"]
struct Resources;

pub fn init(config: &mut Config) {
    config.base = Some(String::from("ArchLinux"));
    config.profile.insert("username".into(), "user".into());
    config
        .profile
        .insert("password".into(), "88Password**".into());
    config.profile.insert("root_password".into(), "root".into());
    config.iso_url = String::from("https://mirrors.edge.kernel.org/archlinux/iso/latest/archlinux-2022.03.01-x86_64.iso");
    config.memory = "";
}

pub fn build(config: &Config, _context: &Path) -> Result<QemuBuilder, Box<dyn Error>> {

    // Create install provisioner
    // TODO

    let root_password = config.profile.get("root_password").unwrap();

    let mut builder = QemuBuilder::new();
    builder.boot_command = vec![
        enter!("passwd"), enter!(root_password), enter!(root_password), // Configure root password
        enter!("systemctl start sshd"), // Start sshd
    ];
    builder.boot_wait = "50s";
    builder.communicator = "ssh".into();
    builder.shutdown_command = "poweroff".into();
    builder.ssh_password = Some("root".into());
    builder.ssh_username = Some("root".into());
    builder.ssh_wait_timeout = Some("5m".into());

    return Ok(builder);
}
