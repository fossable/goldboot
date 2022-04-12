use goldboot_core::cache::MediaCache;
use goldboot_core::qemu::QemuArgs;
use goldboot_core::windows::*;
use goldboot_core::*;
use serde::{Deserialize, Serialize};
use std::error::Error;
use validator::Validate;

#[derive(rust_embed::RustEmbed)]
#[folder = "res/windows_10/"]
struct Resources;

#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct Windows10Template {
    username: String,

    password: String,

    hostname: String,

    iso_url: String,

    iso_checksum: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub provisioners: Option<Vec<Provisioner>>,
}

impl Default for Windows10Template {
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

impl Windows10Template {
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

impl Template for Windows10Template {
    fn build(&self, context: &BuildContext) -> Result<(), Box<dyn Error>> {
        let mut qemuargs = QemuArgs::new(&context);

        qemuargs.drive.push(format!(
            "file={},if=ide,cache=writeback,discard=ignore,format=qcow2",
            context.image_path
        ));
        qemuargs.drive.push(format!(
            "file={},media=cdrom",
            MediaCache::get(self.iso_url.clone(), &self.iso_checksum)?
        ));

        // Write the Autounattend.xml file
        //self.create_unattended().write(&context)?;

        // Copy powershell scripts
        //if let Some(resource) = Resources::get("configure_winrm.ps1") {
        //    std::fs::write(context.join("configure_winrm.ps1"), resource.data)?;
        //}

        // Start VM
        let mut qemu = qemuargs.start_process()?;

        // Send boot command
        #[rustfmt::skip]
        qemu.vnc.boot_command(vec![
            wait!(4),
            enter!(),
        ])?;

        // Wait for SSH
        let ssh = qemu.ssh_wait(context.ssh_port, &self.username, &self.password)?;

        // Run provisioners
        for provisioner in &self.provisioners {
            // TODO
        }

        // Shutdown
        ssh.shutdown("shutdown /s /t 0 /f /d p:4:1")?;
        qemu.shutdown_wait()?;
        Ok(())

        /*builder.floppy_files = Some(vec![
            "Autounattend.xml".into(),
            "configure_winrm.ps1".into(),
        ]);*/
    }
}
