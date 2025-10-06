use crate::{
    build::BuildWorker,
    cache::{MediaCache, MediaFormat},
    provisioners::*,
    qemu::QemuArgs,
    templates::*,
};
use tracing::info;
use serde::{Deserialize, Serialize};
use anyhow::bail;
use std::{
    error::Error,
    io::{BufRead, BufReader},
};
use validator::Validate;

#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct BuildrootTemplate {
    pub source: BuildrootSource,
    pub provisioners: Option<Vec<BuildrootProvisioner>>,
}

pub enum ArchSource {
    Iso(IsoSource),
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ArchProvisioner {
    Ansible(AnsibleProvisioner)
    Mirrorlist(ArchMirrorlistProvisioner),
    Hostname(HostnameProvisioner),
}

impl Default for ArchTemplate {
    fn default() -> Self {
        Self {
            source: None,
            provisioners: None,
        }
    }
}

impl BuildTemplate for ArchTemplate {
    fn build(&self, context: &BuildWorker) -> Result<()> {
        info!("Starting {} build", console::style("ArchLinux").blue());

        let mut qemuargs = QemuArgs::new(&context);

        qemuargs.drive.push(format!(
            "file={},if=virtio,cache=writeback,discard=ignore,format=qcow2",
            context.image_path
        ));
        qemuargs.drive.push(format!(
            "file={},media=cdrom",
            MediaCache::get(self.iso.url.clone(), &self.iso.checksum, MediaFormat::Iso)?
        ));

        // Start VM
        let mut qemu = qemuargs.start_process()?;

        // Send boot command
        #[rustfmt::skip]
		qemu.vnc.boot_command(vec![
			// Initial wait
			wait!(30),
			// Wait for login
			wait_screen_rect!("5b3ca88689e9d671903b3040889c7fa1cb5f244a", 100, 0, 1024, 400),
			// Configure root password
			enter!("passwd"), enter!(self.root_password), enter!(self.root_password),
			// Configure SSH
			enter!("echo 'AcceptEnv *' >>/etc/ssh/sshd_config"),
			enter!("echo 'PermitRootLogin yes' >>/etc/ssh/sshd_config"),
			// Start sshd
			enter!("systemctl restart sshd"),
		])?;

        // Wait for SSH
        let mut ssh = qemu.ssh_wait(context.ssh_port, "root", &self.root_password)?;

        // Run install script
        if let Some(resource) = Resources::get("install.sh") {
            info!("Running base installation");
            match ssh.upload_exec(
                resource.data.to_vec(),
                vec![
                    ("GB_MIRRORLIST", &self.format_mirrorlist()),
                    ("GB_ROOT_PASSWORD", &self.root_password),
                ],
            ) {
                Ok(0) => debug!("Installation completed successfully"),
                _ => bail!("Installation failed"),
            }
        }

        // Run provisioners
        self.provisioners.run(&mut ssh)?;

        // Shutdown
        ssh.shutdown("poweroff")?;
        qemu.shutdown_wait()?;
        Ok(())
    }
}

/// This provisioner configures the Archlinux mirror list.
#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct ArchMirrorlistProvisioner {
    pub mirrors: Vec<String>,
}

impl Default for ArchMirrorlistProvisioner {
    fn default() -> Self {
        Self {
            mirrors: vec![
                String::from("https://geo.mirror.pkgbuild.com/"),
                String::from("https://mirror.rackspace.com/archlinux/"),
                String::from("https://mirrors.edge.kernel.org/archlinux/"),
            ],
        }
    }
}

impl ArchMirrorlistProvisioner {
    pub fn format_mirrorlist(&self) -> String {
        self.mirrors
            .iter()
            .map(|s| format!("Server = {}", s))
            .collect::<Vec<String>>()
            .join("\n")
    }
}

/// Fetch the latest installation ISO
fn fetch_latest_iso(mirrorlist: ArchMirrorlistProvisioner) -> Result<IsoSource> {
    for mirror in mirrorlist.mirrors {
        let rs = reqwest::blocking::get(format!("{mirror}/iso/latest/sha1sums.txt"))?;
        if rs.status().is_success() {
            for line in BufReader::new(rs).lines().filter_map(|result| result.ok()) {
                if line.ends_with(".iso") {
                    let split: Vec<&str> = line.split_whitespace().collect();
                    if let [hash, filename] = split[..] {
                        return Ok(IsoSource {
                            url: format!("{mirror}/iso/latest/{filename}"),
                            checksum: format!("sha1:{hash}"),
                        });
                    }
                }
            }
        }
    }
    bail!("Failed to request latest ISO");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fetch_latest_iso() -> Result<()> {
        fetch_latest_iso(ArchMirrorlistProvisioner::default())?;
        Ok(())
    }
}
