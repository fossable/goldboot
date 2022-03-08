use crate::packer::QemuBuilder;
use crate::Config;
use anyhow::{bail, Result};
use std::path::Path;

pub fn init(config: &mut Config) {
    config.base = Some(String::from("PopOs2104"));
    config.profile.insert("username".into(), "user".into());
    config
        .profile
        .insert("password".into(), "88Password**".into());
    config.profile.insert("root_password".into(), "root".into());
    config.iso_url = "https://pop-iso.sfo2.cdn.digitaloceanspaces.com/21.04/amd64/intel/5/pop-os_21.04_amd64_intel_5.iso";
    config.iso_checksum = Some("sha256:da8448fa5bbed869b146acf3d9315c9c4301d65ebe4cc8a39027f54a73935a43");
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

    let mut builder = QemuBuilder::new();
    builder.boot_command = vec![
        "<enter><wait><enter><wait><enter><wait><enter><wait>".into(),
        "<enter><wait><tab><wait><enter><wait>".into(),
        format!("{username}<tab>{username}<enter><wait>{password}<tab>{password}<enter><wait3>")
            .into(), // Configure user
        "<spacebar><wait><tab><wait><tab><wait><enter><wait116m>".into(), // Start install
        "<tab><wait><enter><wait2m>".into(),                              // Reboot
        format!("<enter>{password}<enter><wait1m>").into(),               // Login after reboot
    ];
    builder.boot_wait = "2m".into();
    builder.communicator = "ssh".into();
    builder.shutdown_command = "poweroff".into();
    builder.ssh_password = Some("root".into());
    builder.ssh_username = Some("root".into());
    builder.ssh_wait_timeout = Some("5m".into());

    return Ok(builder);
}
