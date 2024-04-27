use std::collections::HashMap;

use anyhow::Result;
use dialoguer::theme::Theme;
use goldboot_image::ImageArch;
use serde::{Deserialize, Serialize};
use serde_win_unattend::*;
use tracing::debug;
use validator::Validate;

use crate::{
    cli::prompt::Prompt,
    enter,
    foundry::{
        options::hostname::Hostname,
        qemu::{OsCategory, QemuBuilder},
        sources::ImageSource,
        Foundry, FoundryWorker,
    },
    wait,
};

use super::{CastImage, DefaultSource};

#[derive(Clone, Serialize, Deserialize, Validate, Debug, Default)]
pub struct Windows10 {
    #[serde(flatten)]
    pub hostname: Hostname,

    username: String,

    password: String,
}

// TODO proc macro
impl Prompt for Windows10 {
    fn prompt(&mut self, _foundry: &Foundry, theme: Box<dyn Theme>) -> Result<()> {
        // Prompt for minimal install
        if dialoguer::Confirm::with_theme(&*theme).with_prompt("Perform minimal install? This will remove as many unnecessary programs as possible.").interact()? {

		}
        Ok(())
    }
}

impl DefaultSource for Windows10 {
    fn default_source(&self, _: ImageArch) -> Result<ImageSource> {
        // TODO? https://github.com/pbatard/Fido
        Ok(ImageSource::Iso {
            url: "<TODO>.iso".to_string(),
            checksum: Some("sha256:<TODO>".to_string()),
        })
    }
}

impl CastImage for Windows10 {
    fn cast(&self, worker: &FoundryWorker) -> Result<()> {
        let unattended = UnattendXml {
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
                        value: self.hostname.hostname.clone(),
                    }),
                    DiskConfiguration: None,
                    ImageInstall: None,
                }],
            }],
        };
        let unattended_xml = format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n{}",
            quick_xml::se::to_string(&unattended)?
        );
        debug!(xml = unattended_xml, "Generated Autounattend.xml");

        let mut qemu = QemuBuilder::new(&worker, OsCategory::Windows)
            .source(&worker.element.source)?
            .floppy_files(HashMap::from([(
                "Autounattend.xml".to_string(),
                // unattended_xml.as_bytes().to_vec(),
                include_bytes!("/tmp/Test.xml").into(),
            )]))?
            .prepare_ssh()?
            .start()?;

        // Copy powershell scripts
        //if let Some(resource) = Resources::get("configure_winrm.ps1") {
        //    std::fs::write(context.join("configure_winrm.ps1"), resource.data)?;
        //}

        // Send boot command
        #[rustfmt::skip]
		qemu.vnc.run(vec![
            // Initial wait
			wait!(4),
			enter!(),
		])?;

        // Wait for SSH
        // let mut ssh = qemu.ssh_wait(context.ssh_port, &self.username, &self.password)?;

        // Shutdown
        // ssh.shutdown("shutdown /s /t 0 /f /d p:4:1")?;
        qemu.shutdown_wait()?;
        Ok(())
    }
}
