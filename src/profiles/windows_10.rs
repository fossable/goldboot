use crate::{
    config::Config,
    packer::PackerTemplate,
    profile::Profile,
    windows::{Component, ComputerName, Settings, UnattendXml},
};
use rust_embed::RustEmbed;
use std::{error::Error, path::Path};
use validator::Validate;

#[derive(RustEmbed)]
#[folder = "res/windows_10/"]
struct Resources;

#[derive(Validate)]
pub struct Windows10Profile {
    username: String,
    password: String,
    hostname: String,
}

impl Windows10Profile {
    pub fn new(config: &mut Config) -> Result<Self, Box<dyn Error>> {
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
            hostname: config
                .profile
                .get("hostname")
                .ok_or("Missing hostname")?
                .to_string(),
        };

        profile.validate()?;
        Ok(profile)
    }

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

pub fn init(config: &mut Config) {
    config.base = Some(String::from("Windows10"));
    config.profile.insert("username".into(), "admin".into());
    config.profile.insert("password".into(), "admin".into());
    config.iso_url = String::from("<ISO URL>");
    config.iso_checksum = Some(String::from("<ISO checksum>"));
}

impl Profile for Windows10Profile {
    fn build(&self, template: &mut PackerTemplate, context: &Path) -> Result<(), Box<dyn Error>> {
        // Write the Autounattend.xml file
        self.create_unattended().write(&context)?;

        // Copy powershell scripts
        if let Some(resource) = Resources::get("configure_winrm.ps1") {
            std::fs::write(context.join("configure_winrm.ps1"), resource.data)?;
        }

        let builder = template.builders.first_mut().unwrap();
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

        Ok(())
    }
}
