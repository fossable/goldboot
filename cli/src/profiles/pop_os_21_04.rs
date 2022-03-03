use crate::packer::QemuBuilder;
use crate::Config;
use anyhow::Result;
use std::path::Path;

pub fn init(config: &mut Config) {
    config.user.username = String::from("user");
    config.user.password = String::from("88Password**");
    config.iso_url = String::from(
        "https://pop-iso.sfo2.cdn.digitaloceanspaces.com/21.04/amd64/intel/5/pop-os_21.04_amd64_intel_5.iso",
    );
    config.iso_checksum = Some(String::from(
        "sha256:da8448fa5bbed869b146acf3d9315c9c4301d65ebe4cc8a39027f54a73935a43",
    ));
}

pub fn build(config: &Config, _context: &Path) -> Result<QemuBuilder> {
    let mut builder = QemuBuilder::new();
    builder.boot_command = vec![
        "<enter><wait><enter><wait><enter><wait><enter><wait>".into(),
        "<enter><wait><tab><wait><enter><wait>".into(),
        format!(
            "{}<tab>{}<enter><wait>{}<tab>{}<enter><wait3>",
            config.user.username, config.user.username, config.user.password, config.user.password
        )
        .into(),
        "<spacebar><wait><tab><wait><tab><wait><enter><wait6m>".into(),
        "<tab><wait><tab><wait><enter>".into(),
    ];
    builder.boot_wait = "2m".into();
    builder.communicator = "none".into();
    builder.headless = false;

    return Ok(builder);
}
