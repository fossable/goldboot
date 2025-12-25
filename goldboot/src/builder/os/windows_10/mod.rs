use std::collections::HashMap;

use anyhow::Result;
use goldboot_image::ImageArch;
use serde::{Deserialize, Serialize};
use serde_win_unattend::*;
use smart_default::SmartDefault;
use tracing::debug;
use validator::Validate;

use crate::{
    builder::{
        Builder,
        options::{arch::Arch, hostname::Hostname, iso::Iso, size::Size},
        qemu::{OsCategory, QemuBuilder},
    },
    cli::prompt::Prompt,
    enter, wait,
};

use super::BuildImage;

/// Windows 10 is a major release of Microsoft's Windows NT operating system.
///
/// Upstream: https://microsoft.com
/// Maintainer: cilki
#[derive(Clone, Serialize, Deserialize, Validate, Debug, SmartDefault, goldboot_macros::Prompt, goldboot_macros::StarlarkConstructor)]
pub struct Windows10 {
    #[default(Arch(ImageArch::Amd64))]
    pub arch: Arch,
    pub size: Size,
    #[serde(flatten)]
    pub hostname: Hostname,

    // username: String,

    // password: String,
    #[default(Iso {
        url: "http://example.com".parse().unwrap(),
        checksum: None,
    })]
    iso: Iso,
}

impl BuildImage for Windows10 {
    fn build(&self, worker: &Builder) -> Result<()> {
        let unattended = UnattendXml {
            xmlns: "urn:schemas-microsoft-com:unattend".into(),
            settings: vec![
                Settings {
                    pass: "windowsPE".into(),
                    component: vec![
                        Component {
                            name: "Microsoft-Windows-International-Core-WinPE".into(),
                            UILanguage: Some("en-US".into()),
                            UserLocale: Some("en-US".into()),
                            SystemLocale: Some("en-US".into()),
                            InputLocale: Some("en-US".into()),
                            SetupUILanguage: Some(SetupUILanguage {
                                UILanguage: "en-US".into(),
                            }),
                            ..Default::default()
                        },
                        Component {
                            name: "Microsoft-Windows-Setup".into(),
                            DiskConfiguration: Some(DiskConfiguration {
                                WillShowUI: None,
                                Disk: Disk {
                                    CreatePartitions: CreatePartitions {
                                        CreatePartition: vec![
                                            CreatePartition {
                                                Order: "1".into(),
                                                Size: Some("100".into()),
                                                Extend: None,
                                                Type: "Primary".into(),
                                            },
                                            CreatePartition {
                                                Order: "2".into(),
                                                Size: None,
                                                Extend: None,
                                                Type: "Primary".into(),
                                            },
                                        ],
                                    },
                                    ModifyPartitions: ModifyPartitions {
                                        ModifyPartition: vec![
                                            ModifyPartition {
                                                Format: "NTFS".into(),
                                                Label: "System".into(),
                                                Order: "1".into(),
                                                PartitionID: "1".into(),
                                                Letter: None,
                                            },
                                            ModifyPartition {
                                                Format: "NTFS".into(),
                                                Label: "OS".into(),
                                                Order: "2".into(),
                                                PartitionID: "2".into(),
                                                Letter: Some("C".into()),
                                            },
                                        ],
                                    },
                                    WillWipeDisk: "false".into(),
                                    DiskID: "0".into(),
                                },
                            }),
                            ImageInstall: Some(ImageInstall {
                                OSImage: OSImage {
                                    InstallTo: Some(InstallTo {
                                        DiskID: "0".into(),
                                        PartitionID: "2".into(),
                                    }),
                                    WillShowUI: Some("Never".into()),
                                    InstallToAvailablePartition: None,
                                },
                            }),
                            UserData: Some(UserData {
                                AcceptEula: "true".into(),
                                FullName: "test".into(),
                                Organization: "test".into(),
                                ProductKey: ProductKey {
                                    Key: "W269N-WFGWX-YVC9B-4J6C9-T83GX".into(),
                                    WillShowUI: Some("Never".into()),
                                },
                            }),
                            ..Default::default()
                        },
                    ],
                },
                Settings {
                    pass: "specialize".into(),
                    component: vec![Component {
                        name: "Microsoft-Windows-Shell-Setup".into(),
                        ComputerName: Some(self.hostname.hostname.clone()),
                        ..Default::default()
                    }],
                },
            ],
        };
        let unattended_xml = format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n{}",
            quick_xml::se::to_string(&unattended)?
        );
        debug!(xml = unattended_xml, "Generated Autounattend.xml");

        let mut qemu = QemuBuilder::new(&worker, OsCategory::Windows)
            .with_iso(&self.iso)?
            .floppy_files(HashMap::from([(
                "Autounattend.xml".to_string(),
                unattended_xml.as_bytes().to_vec(),
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
