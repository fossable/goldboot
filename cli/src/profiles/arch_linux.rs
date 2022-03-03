use crate::packer::QemuBuilder;
use crate::Config;
use anyhow::Result;
use rust_embed::RustEmbed;
use std::path::Path;

#[derive(RustEmbed)]
#[folder = "res/arch_linux/"]
struct Resources;

pub fn init(config: &mut Config) {
    config.user.username = String::from("root");
    config.user.password = String::from("root");
    config.iso_url = String::from(
        "https://mirrors.edge.kernel.org/archlinux/iso/2021.10.01/archlinux-2021.10.01-x86_64.iso",
    );
    config.iso_checksum = Some(String::from(
        "sha1:77a20dcd9d838398cebb2c7c15f46946bdc3855e",
    ));
}

pub fn build(_config: &Config, _context: &Path) -> Result<QemuBuilder> {
    let mut builder = QemuBuilder::new();
    builder.boot_command = vec![
        "echo root:root | chpasswd<enter><wait2>".into(),
        "systemctl start sshd<enter>".into(),
    ];
    builder.boot_wait = "50s".into();
    builder.communicator = "ssh".into();
    builder.shutdown_command = "poweroff".into();
    builder.ssh_password = Some("root".into());
    builder.ssh_username = Some("root".into());
    builder.ssh_wait_timeout = Some("5m".into());

    return Ok(builder);
}
