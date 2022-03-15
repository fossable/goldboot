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

#[derive(Clone, Serialize, Deserialize, Validate)]
pub struct ArchLinuxProfile {
    pub root_password: String,

    pub mirrorlist: Vec<String>,

    /// The installation media URL
    pub iso_url: String,

    /// A hash of the installation media
    pub iso_checksum: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub partitions: Option<Vec<Partition>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub provisioners: Option<Vec<Provisioner>>,
}

impl ArchLinuxProfile {
    pub fn format_mirrorlist(&self) -> String {
        self.mirrorlist.iter().map(|s| format!("Server = {}", s)).collect::<Vec<String>>().join("\n")
    }
}

impl Default for ArchLinuxProfile {
    fn default() -> Self {
        Self {
            root_password: String::from("root"),
            mirrorlist: vec![
                String::from("https://mirrors.kernel.org/archlinux/$repo/os/$arch")
            ],
            iso_url: String::from("https://mirrors.edge.kernel.org/archlinux/iso/latest/archlinux-2022.03.01-x86_64.iso"),
            iso_checksum: String::from("none"),
            partitions: None,
            provisioners: None,
        }
    }
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
                environment_vars: Some(vec![
                    format!("GB_MIRRORLIST={}", self.format_mirrorlist()),
                    format!("GB_ROOT_PASSWORD={}", self.root_password),
                ]),
            }),
        });

        // Add user provisioners
        if let Some(provisioners) = &self.provisioners {
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
        builder.communicator = String::from("ssh");
        builder.shutdown_command = String::from("poweroff");
        builder.ssh_password = Some(String::from("root"));
        builder.ssh_username = Some(String::from("root"));
        builder.ssh_wait_timeout = Some(String::from("1m"));
        builder.iso_url = self.iso_url.clone();
        builder.iso_checksum = self.iso_checksum.clone();
        builder.qemuargs = vec![
            vec![String::from("-drive"), String::from("if=pflash,format=raw,unit=0,readonly=on,file=/usr/share/ovmf/x64/OVMF.fd")],
            //vec![String::from("-drive"), format!("if=pflash,format=raw,unit=1,file={}/OVMF_VARS.fd", context.to_string_lossy())],
            vec![String::from("-drive"), format!("if=pflash,format=raw,unit=1,file=/usr/share/ovmf/x64/OVMF_VARS.fd")],
            vec![String::from("-drive"), format!("file=/var/lib/goldboot/images/output/goldboot,if=virtio,cache=writeback,discard=ignore,format=qcow2")],
            vec![String::from("-drive"), format!("file={},media=cdrom", builder.iso_path().to_string_lossy())]
        ];

        template.builders.push(builder);

        Ok(template)
    }
}
