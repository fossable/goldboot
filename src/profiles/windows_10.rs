use crate::cache::MediaCache;
use crate::config::Config;
use crate::config::Provisioner;
use crate::qemu::QemuArgs;
use crate::{
    profile::Profile,
    vnc::bootcmds::{enter, wait},
    windows::{Component, ComputerName, Settings, UnattendXml},
};
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use std::error::Error;
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

    #[serde(skip_serializing_if = "Option::is_none")]
    pub provisioners: Option<Vec<Provisioner>>,
}

impl Default for Windows10Profile {
    fn default() -> Self {
        Self {
            username: String::from("admin"),
            password: String::from("admin"),
            hostname: String::from("goldboot"),
            iso_url: String::from("<ISO URL>"),
            iso_checksum: String::from("<ISO HASH>"),
            provisioners: None,
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
    fn build(&self, config: &Config, image_path: &str) -> Result<(), Box<dyn Error>> {
        let mut qemuargs = QemuArgs::new(&config);

        qemuargs.add_drive(image_path, "ide");
        qemuargs.add_cdrom(MediaCache::get(self.iso_url.clone(), &self.iso_checksum)?);

        // Write the Autounattend.xml file
        //self.create_unattended().write(&context)?;

        // Copy powershell scripts
        //if let Some(resource) = Resources::get("configure_winrm.ps1") {
        //    std::fs::write(context.join("configure_winrm.ps1"), resource.data)?;
        //}

        // Start VM
        let mut qemu = qemuargs.start_process()?;

        // Send boot command
        qemu.vnc.boot_command(vec![
            wait!(4), // Wait for boot
            enter!(),
        ]);

        // Wait for SSH
        let ssh = qemu.ssh()?;

        // Run provisioners
        for provisioner in &self.provisioners {
            // TODO
        }

        // Shutdown
        qemu.shutdown("shutdown /s /t 0 /f /d p:4:1")?;
        Ok(())

        /*builder.floppy_files = Some(vec![
            "Autounattend.xml".into(),
            "configure_winrm.ps1".into(),
        ]);*/
    }
}
