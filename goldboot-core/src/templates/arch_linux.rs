use colored::*;
use crate::cache::MediaCache;
use crate::qemu::QemuArgs;
use crate::templates::*;
use log::info;
use serde::{Deserialize, Serialize};
use simple_error::bail;
use std::{error::Error, io::BufRead, io::BufReader};
use validator::Validate;

const DEFAULT_MIRROR: &str = "https://mirrors.edge.kernel.org/archlinux";

#[derive(rust_embed::RustEmbed)]
#[folder = "res/arch_linux/"]
struct Resources;

#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct ArchLinuxTemplate {
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

impl ArchLinuxTemplate {
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

impl Default for ArchLinuxTemplate {
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

impl Template for ArchLinuxTemplate {
    fn build(&self, context: &BuildContext) -> Result<(), Box<dyn Error>> {
        info!("Starting {} build", "ArchLinux".blue());

        let mut qemuargs = QemuArgs::new(&context);

        qemuargs.drive.push(format!(
            "file={},if=virtio,cache=writeback,discard=ignore,format=qcow2",
            context.image_path
        ));
        qemuargs.drive.push(format!(
            "file={},media=cdrom",
            MediaCache::get(self.iso_url.clone(), &self.iso_checksum)?
        ));

        // Start VM
        let mut qemu = qemuargs.start_process()?;

        // Send boot command
        #[rustfmt::skip]
        qemu.vnc.boot_command(vec![
            // Initial wait
            wait!(60),
            // Wait for login
            wait_screen_rect!("426f88982ab5cb075a9e59578d06e9c28530e43c", 100, 0, 1024, 400),
            // Configure root password
            enter!("passwd"), enter!(self.root_password), enter!(self.root_password),
            // Start sshd
            enter!("systemctl restart sshd"),
        ])?;

        // Wait for SSH
        let ssh = qemu.ssh_wait(context.ssh_port, "root", &self.root_password)?;

        // Run install script
        if let Some(mut resource) = Resources::get("install.sh") {
            ssh.upload_exec(
                resource.data.to_vec(),
                vec![
                    format!("GB_MIRRORLIST={}", self.format_mirrorlist()),
                    format!("GB_ROOT_PASSWORD={}", self.root_password),
                ],
            )?;
        }

        // Run provisioners
        if let Some(provisioners) = &self.provisioners {
            for provisioner in provisioners {
                provisioner.run(&ssh)?;
            }
        }

        // Shutdown
        ssh.shutdown("poweroff")?;
        qemu.shutdown_wait()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fetch_latest_iso() -> Result<(), Box<dyn Error>> {
        fetch_latest_iso()?;
        Ok(())
    }
}
