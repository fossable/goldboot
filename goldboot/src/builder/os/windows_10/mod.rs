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
        options::{
            arch::Arch,
            hostname::Hostname,
            iso::Iso,
            locale::Locale,
            size::Size,
            timezone::Timezone,
            unix_users::UnixUsers,
        },
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
#[goldboot_macros::Os(architectures(Amd64))]
#[derive(Clone, Serialize, Deserialize, Validate, Debug, SmartDefault, goldboot_macros::Prompt)]
pub struct Windows10 {
    #[default(Arch(ImageArch::Amd64))]
    pub arch: Arch,
    pub size: Size,
    #[serde(default)]
    pub hostname: Hostname,

    /// Locale and keyboard settings
    #[serde(default)]
    pub locale: Locale,

    /// System timezone in Windows format (e.g. "UTC", "Eastern Standard Time")
    #[serde(default)]
    pub timezone: Timezone,

    /// Additional local user accounts to create
    pub users: Option<UnixUsers>,

    /// Windows product key (leave unset to use a generic/evaluation key)
    pub product_key: Option<WindowsProductKey>,

    #[default(Iso {
        url: "http://example.com".parse().unwrap(),
        checksum: None,
    })]
    pub iso: Iso,
}

impl Windows10 {
    fn generate_unattend(&self) -> Result<String> {
        let product_key = self
            .product_key
            .as_ref()
            .map(|k| k.0.clone())
            // Generic/KMS key for Windows 10 Pro
            .unwrap_or_else(|| "W269N-WFGWX-YVC9B-4J6C9-T83GX".to_string());

        let locale_tag = locale_to_windows_tag(&self.locale);

        let mut first_logon_commands = vec![
            // Set timezone
            SynchronousCommand {
                CommandLine: format!(
                    r#"powershell -Command "Set-TimeZone -Id '{}'""#,
                    self.timezone.0
                ),
                Description: Some("Set timezone".into()),
                Order: "1".into(),
                RequiresUserInput: None,
                action: None,
            },
        ];

        // Create extra users
        if let Some(users) = &self.users {
            for (i, user) in users.0.iter().enumerate() {
                let order = (i + 2).to_string();
                first_logon_commands.push(SynchronousCommand {
                    CommandLine: format!(
                        r#"powershell -Command "New-LocalUser -Name '{}' -Password (ConvertTo-SecureString '{}' -AsPlainText -Force) -FullName '{}' -PasswordNeverExpires""#,
                        user.username, user.password, user.username
                    ),
                    Description: Some(format!("Create user {}", user.username)),
                    Order: order.clone(),
                    RequiresUserInput: None,
                    action: None,
                });
                if user.sudo {
                    first_logon_commands.push(SynchronousCommand {
                        CommandLine: format!(
                            r#"powershell -Command "Add-LocalGroupMember -Group 'Administrators' -Member '{}'""#,
                            user.username
                        ),
                        Description: Some(format!("Add {} to Administrators", user.username)),
                        Order: format!("{}.1", order),
                        RequiresUserInput: None,
                        action: None,
                    });
                }
            }
        }

        let unattended = UnattendXml {
            xmlns: "urn:schemas-microsoft-com:unattend".into(),
            settings: vec![
                Settings {
                    pass: "windowsPE".into(),
                    component: vec![
                        Component {
                            name: "Microsoft-Windows-International-Core-WinPE".into(),
                            UILanguage: Some(locale_tag.clone()),
                            UserLocale: Some(locale_tag.clone()),
                            SystemLocale: Some(locale_tag.clone()),
                            InputLocale: Some(self.locale.keyboard.clone()),
                            SetupUILanguage: Some(SetupUILanguage {
                                UILanguage: locale_tag.clone(),
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
                                FullName: "goldboot".into(),
                                Organization: "goldboot".into(),
                                ProductKey: ProductKey {
                                    Key: product_key,
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
                Settings {
                    pass: "oobeSystem".into(),
                    component: vec![Component {
                        name: "Microsoft-Windows-Shell-Setup".into(),
                        FirstLogonCommands: Some(FirstLogonCommands {
                            SynchronousCommand: first_logon_commands,
                        }),
                        ..Default::default()
                    }],
                },
            ],
        };

        Ok(format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n{}",
            quick_xml::se::to_string(&unattended)?
        ))
    }
}

impl BuildImage for Windows10 {
    fn build(&self, worker: &Builder) -> Result<()> {
        let unattended_xml = self.generate_unattend()?;
        debug!(xml = unattended_xml, "Generated Autounattend.xml");

        let mut qemu = QemuBuilder::new(&worker, OsCategory::Windows)
            .with_iso(&self.iso)?
            .floppy_files(HashMap::from([(
                "Autounattend.xml".to_string(),
                unattended_xml.as_bytes().to_vec(),
            )]))?
            .prepare_ssh()?
            .start()?;

        // Send boot command
        #[rustfmt::skip]
        qemu.vnc.run(vec![
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

/// Windows product key.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct WindowsProductKey(pub String);

impl Prompt for WindowsProductKey {
    fn prompt(&mut self, _: &Builder) -> Result<()> {
        Ok(())
    }
}

/// Convert a IETF locale (e.g. "en_US") to a Windows locale tag (e.g. "en-US").
fn locale_to_windows_tag(locale: &Locale) -> String {
    locale.language.replace('_', "-")
}
