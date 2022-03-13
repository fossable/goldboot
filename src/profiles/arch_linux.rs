use crate::{
    config::{Partition, Provisioner},
    packer::bootcmds::enter,
    packer::{PackerProvisioner, PackerTemplate, QemuBuilder, ShellPackerProvisioner},
    profile::Profile,
};
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use std::{error::Error, path::Path};
use validator::Validate;

#[derive(RustEmbed)]
#[folder = "res/arch_linux/"]
struct Resources;

#[derive(Clone, Serialize, Deserialize, Validate, Default)]
pub struct ArchLinuxProfile {
    #[serde(default = "default_root_password")]
    pub root_password: String,

    #[serde(default = "default_mirrorlist")]
    pub mirrorlist: Vec<String>,

    /// The installation media URL
    pub iso_url: String,

    /// A hash of the installation media
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iso_checksum: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub partitions: Option<Vec<Partition>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub provisioners: Option<Vec<Provisioner>>,
}

fn default_root_password() -> String {
    String::from("root")
}

fn default_mirrorlist() -> Vec<String> {
    vec![String::from(
        "https://mirrors.kernel.org/archlinux/$repo/os/$arch",
    )]
}

fn default_iso_url() -> String {
    String::from(
        "https://mirrors.edge.kernel.org/archlinux/iso/latest/archlinux-2022.03.01-x86_64.iso",
    )
}

impl Profile for ArchLinuxProfile {
    fn generate_template(&self, context: &Path) -> Result<PackerTemplate, Box<dyn Error>> {
        let mut template = PackerTemplate::default();

        // Create install provisioner
        template.provisioners.push(PackerProvisioner {
            r#type: String::from("shell"),
            ansible: None,
            shell: Some(ShellPackerProvisioner {
                scripts: Some(vec![String::from("install.sh")]),
                expect_disconnect: Some(true),
            }),
        });

        // Add user provisioners
        if let Some(provisioners) = self.provisioners {
            provisioners
                .iter()
                .map(|p| p.into())
                .for_each(|p| template.provisioners.push(p));
        }

        // Copy scripts
        if let Some(resource) = Resources::get("install.sh") {
            std::fs::write(context.join("install.sh"), resource.data)?;
        }

        let mut builder = QemuBuilder::new();
        builder.boot_command = vec![
            enter!("passwd"),
            enter!(self.root_password),
            enter!(self.root_password),     // Configure root password
            enter!("systemctl start sshd"), // Start sshd
        ];
        builder.boot_wait = String::from("50s");
        builder.communicator = "ssh".into();
        builder.shutdown_command = "poweroff".into();
        builder.ssh_password = Some("root".into());
        builder.ssh_username = Some("root".into());
        builder.ssh_wait_timeout = Some("5m".into());

        template.builders.push(builder);

        Ok(template)
    }
}
