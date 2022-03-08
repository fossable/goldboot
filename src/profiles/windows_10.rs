use crate::{
    config::Config,
    packer::QemuBuilder,
    windows::{Component, ComputerName, Settings, UnattendXml},
};
use rust_embed::RustEmbed;
use std::{
    path::Path,
    error::Error,
};

#[derive(RustEmbed)]
#[folder = "res/windows_10/"]
struct Resources;

pub fn init(config: &mut Config) {
    config.base = Some(String::from("Windows10"));
    config.profile.insert("username".into(), "admin".into());
    config.profile.insert("password".into(), "admin".into());
    config.iso_url = "<ISO URL>";
    config.iso_checksum = Some("<ISO checksum>");
}

fn create_unattended(config: &Config) -> UnattendXml {
    UnattendXml {
        xmlns: "urn:schemas-microsoft-com:unattend".into(),
        settings: vec![Settings {
            pass: "specialize".into(),
            component: vec![Component {
                name: "Microsoft-Windows-Shell-Setup".into(),
                processorArchitecture: "amd64".into(),
                publicKeyToken: "31bf3856ad364e35".into(),
                language: "neutral".into(),
                versionScope: "nonSxS".into(),
                ComputerName: Some(ComputerName {
                    value: config.name.to_string(),
                }),
                DiskConfiguration: None,
                ImageInstall: None,
            }],
        }],
    }
}

pub fn build(config: &Config, context: &Path) -> Result<QemuBuilder, Box<dyn Error>> {
    // Write the Autounattend.xml file
    create_unattended(&config).write(&context)?;

    // Copy powershell scripts
    if let Some(resource) = Resources::get("configure_winrm.ps1") {
        std::fs::write(context.join("configure_winrm.ps1"), resource.data).unwrap();
    }

    // Create the initial builder
    let mut builder = QemuBuilder::new();
    builder.boot_command = vec!["<enter>".into()];
    builder.boot_wait = "4s";
    builder.shutdown_command = "shutdown /s /t 0 /f /d p:4:1 /c \"Packer Shutdown\"".into();
    builder.communicator = "winrm".into();
    builder.winrm_insecure = Some(true);
    builder.winrm_timeout = Some("2h".into());
    builder.disk_interface = "ide";
    builder.floppy_files = Some(vec![
        "Autounattend.xml".into(),
        "configure_winrm.ps1".into(),
    ]);

    return Ok(builder);
}
