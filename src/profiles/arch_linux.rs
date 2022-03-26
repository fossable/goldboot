use crate::cache::MediaCache;
use crate::config::Config;
use crate::qemu::QemuArgs;
use crate::{
    config::{Partition, Provisioner},
    profile::Profile,
    vnc::bootcmds::{enter, wait},
};
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use simple_error::bail;
use std::{error::Error, io::BufRead, io::BufReader};
use validator::Validate;
use log::info;
use colored::*;

const DEFAULT_MIRROR: &str = "https://mirrors.edge.kernel.org/archlinux";

#[derive(RustEmbed)]
#[folder = "res/arch_linux/"]
struct Resources;

#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct ArchLinuxProfile {
    #[validate(length(max = 64))]
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
        self.mirrorlist
            .iter()
            .map(|s| format!("Server = {}", s))
            .collect::<Vec<String>>()
            .join("\n")
    }
}

/// Fetch the latest iso URL and its SHA1 hash
pub fn fetch_latest_iso() -> Result<(String, String), Box<dyn Error>> {
    let rs = reqwest::blocking::get(format!("{DEFAULT_MIRROR}/iso/latest/sha1sums.txt"))?;
    if rs.status().is_success() {
        for line in BufReader::new(rs).lines().filter_map(|result| result.ok()) {
            if line.ends_with(".iso") {
                let split: Vec<&str> = line.split_whitespace().collect();
                if let [hash, filename] = split[..] {
                    return Ok((
                        format!("{DEFAULT_MIRROR}/iso/latest/{filename}"),
                        format!("sha1:{hash}"),
                    ));
                }
            }
        }
    }
    bail!("Failed to request latest ISO");
}

impl Default for ArchLinuxProfile {
    fn default() -> Self {
        let (iso_url, iso_checksum) = fetch_latest_iso().unwrap_or((
            format!("{DEFAULT_MIRROR}/iso/latest/archlinux-2022.03.01-x86_64.iso"),
            String::from("none"),
        ));
        Self {
            root_password: String::from("root"),
            mirrorlist: vec![format!("{DEFAULT_MIRROR}/$repo/os/$arch",)],
            iso_url: iso_url,
            iso_checksum: iso_checksum,
            partitions: None,
            provisioners: None,
        }
    }
}

/*impl Profile for ArchLinuxProfile {
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
    }
}*/

impl Profile for ArchLinuxProfile {
    fn build(
        &self,
        config: &Config,
        image_path: &str,
    ) -> Result<(), Box<dyn Error>> {
        info!("Starting {} build", "ArchLinux".blue());

        let mut qemuargs = QemuArgs::new(&config);

        qemuargs.drive.push(format!(
            "file={image_path},if=virtio,cache=writeback,discard=ignore,format=qcow2"
        ));
        qemuargs.drive.push(format!(
            "file={},media=cdrom",
            MediaCache::get(self.iso_url.clone(), &self.iso_checksum)?
        ));

        // Start VM
        let mut qemu = qemuargs.start_process()?;

        // Send boot command
        qemu.vnc.boot_command(vec![
            wait!(60), // Wait for boot
            enter!("passwd"),
            enter!(self.root_password),
            enter!(self.root_password),     // Configure root password
            enter!("systemctl start sshd"), // Start sshd
        ])?;

        // Wait for SSH
        let ssh = qemu.ssh()?;

        // Run provisioners
        for provisioner in &self.provisioners {
            // TODO
        }

        // Shutdown
        qemu.shutdown("poweroff")?;
        Ok(())
    }
}
