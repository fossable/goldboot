use crate::{
    packer::{PackerTemplate, QemuBuilder},
    profile::Profile,
    windows::{Component, ComputerName, Settings, UnattendXml},
};
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use std::{error::Error, path::Path};
use validator::Validate;

#[derive(RustEmbed)]
#[folder = "res/windows_10/"]
struct Resources;

#[derive(Clone, Serialize, Deserialize, Validate)]
pub struct Windows10Profile {
    username: String,

    password: String,

    hostname: String,

    iso_url: String,

    iso_checksum: String,
}

impl Default for Windows10Profile {
    fn default() -> Self {
        Self {
            username: String::from("admin"),
            password: String::from("admin"),
            hostname: String::from("goldboot"),
            iso_url: String::from("<ISO URL>"),
            iso_checksum: String::from("<ISO HASH>"),
        }
    }
}

impl Windows10Profile {
    fn create_unattended(&self) -> UnattendXml {
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
                        value: self.hostname.clone(),
                    }),
                    DiskConfiguration: None,
                    ImageInstall: None,
                }],
            }],
        }
    }
}

impl Profile for Windows10Profile {
    fn generate_template(&self, context: &Path) -> Result<PackerTemplate, Box<dyn Error>> {
        let mut template = PackerTemplate::default();

        // Write the Autounattend.xml file
        self.create_unattended().write(&context)?;

        // Copy powershell scripts
        if let Some(resource) = Resources::get("configure_winrm.ps1") {
            std::fs::write(context.join("configure_winrm.ps1"), resource.data)?;
        }

        let mut builder = QemuBuilder::new();
        builder.boot_command = vec!["<enter>".into()];
        builder.boot_wait = String::from("4s");
        builder.shutdown_command = "shutdown /s /t 0 /f /d p:4:1 /c \"Packer Shutdown\"".into();
        builder.communicator = "winrm".into();
        builder.winrm_insecure = Some(true);
        builder.winrm_timeout = Some("2h".into());
        builder.disk_interface = String::from("ide");
        builder.floppy_files = Some(vec![
            "Autounattend.xml".into(),
            "configure_winrm.ps1".into(),
        ]);
        template.builders.push(builder);

        Ok(template)
    }
}
