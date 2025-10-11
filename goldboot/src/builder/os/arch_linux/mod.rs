use super::BuildImage;
use crate::builder::fabricators::Fabricate;
use crate::builder::http::HttpServer;
use crate::builder::options::hostname::Hostname;
use crate::builder::options::iso::Iso;
use crate::builder::options::unix_account::RootPassword;
use crate::builder::os::arch_linux::archinstall::ArchinstallConfig;
use crate::builder::os::arch_linux::archinstall::ArchinstallCredentials;
use crate::builder::qemu::{OsCategory, QemuBuilder};
use crate::cli::prompt::Prompt;
use crate::wait;
use crate::{builder::Builder, wait_screen_rect};
use anyhow::Result;
use anyhow::bail;
use goldboot_image::ImageArch;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader};
use tracing::{debug, info};
use validator::Validate;

mod archinstall;

/// Arch Linux is an independently developed x86-64 general-purpose Linux distribution
/// that strives to provide the latest stable versions of most software by following
/// a rolling-release model.
///
/// Upstream: https://archlinux.org
/// Maintainer: cilki
#[derive(Clone, Serialize, Deserialize, Validate, Debug, goldboot_macros::Prompt)]
pub struct ArchLinux {
    #[serde(flatten)]
    pub hostname: Hostname,
    pub mirrorlist: Option<ArchLinuxMirrorlist>,
    #[serde(flatten)]
    pub packages: Option<ArchLinuxPackages>,
    pub root_password: RootPassword,
    pub iso: Iso,
}

impl Default for ArchLinux {
    fn default() -> Self {
        Self {
            hostname: Hostname::default(),
            mirrorlist: None,
            packages: None,
            root_password: RootPassword::default(),
            iso: Iso {
                url: "http://mirrors.edge.kernel.org/archlinux/iso/latest/archlinux-2025.10.01-x86_64.iso".parse().unwrap(),
                checksum: None,
            },
        }
    }
}

impl BuildImage for ArchLinux {
    fn build(&self, worker: &Builder) -> Result<()> {
        let mut qemu = QemuBuilder::new(&worker, OsCategory::Linux)
            .with_iso(&self.iso)?
            .prepare_ssh()?
            .start()?;

        // Generate an archinstall config
        let archinstall_config = ArchinstallConfig::from(self);
        debug!(archinstall = ?archinstall_config, "Preparing archinstall config");

        let archinstall_creds = ArchinstallCredentials::from(self);

        // Start HTTP
        let http = HttpServer::new()?
            .file("config.json", serde_json::to_vec(&archinstall_config)?)?
            .file("creds.json", serde_json::to_vec(&archinstall_creds)?)?
            .serve();

        // Send boot command
        #[rustfmt::skip]
		qemu.vnc.run(vec![
			// Initial wait
			wait!(30),
			// Wait for login
			wait_screen_rect!("5b3ca88689e9d671903b3040889c7fa1cb5f244a", 100, 0, 1024, 400),
		])?;

        // Wait for SSH
        let mut ssh = qemu.ssh("root")?;

        // Run install script
        info!("Running base installation");
        match ssh.upload_exec(
            include_bytes!("bootstrap.sh"),
            vec![
                ("GB_HTTP_HOST", &http.address),
                ("GB_HTTP_PORT", &format!("{}", &http.port)),
            ],
        ) {
            Ok(0) => debug!("Installation completed successfully"),
            _ => bail!("Installation failed"),
        }

        // Run remaining fabricators
        // if let Some(fabricators) = &worker.element.fabricators {
        //     for fabricator in fabricators {
        //         fabricator.run(&mut ssh)?;
        //     }
        // }

        // Shutdown
        ssh.shutdown("poweroff")?;
        qemu.shutdown_wait()?;
        Ok(())
    }
}

/// This provisioner configures the Archlinux mirror list.
#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct ArchLinuxMirrorlist {
    pub mirrors: Vec<String>,
}

//https://archlinux.org/mirrorlist/?country=US&protocol=http&protocol=https&ip_version=4

impl Default for ArchLinuxMirrorlist {
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

impl Prompt for ArchLinuxMirrorlist {
    fn prompt(&mut self, _: &Builder) -> Result<()> {
        // Prompt mirror list
        // {
        //     let mirror_index = dialoguer::Select::with_theme(&theme)
        //         .with_prompt("Choose a mirror site")
        //         .default(0)
        //         .items(&MIRRORLIST)
        //         .interact()?;

        //     self.mirrors = vec![MIRRORLIST[mirror_index].to_string()];
        // }

        Ok(())
    }
}

impl ArchLinuxMirrorlist {
    pub fn format_mirrorlist(&self) -> String {
        self.mirrors
            .iter()
            .map(|s| format!("Server = {}", s))
            .collect::<Vec<String>>()
            .join("\n")
    }
}

/// Fetch the latest installation ISO
fn fetch_latest_iso() -> Result<Iso> {
    let rs = reqwest::blocking::get(format!(
        "http://mirrors.edge.kernel.org/archlinux/iso/latest/sha256sums.txt"
    ))?;
    if rs.status().is_success() {
        for line in BufReader::new(rs).lines().filter_map(|result| result.ok()) {
            if line.ends_with(".iso") {
                let split: Vec<&str> = line.split_whitespace().collect();
                if let [hash, filename] = split[..] {
                    return Ok(Iso {
                        url: format!(
                            "http://mirrors.edge.kernel.org/archlinux/iso/latest/{filename}"
                        )
                        .parse()
                        .unwrap(),
                        checksum: Some(format!("sha256:{hash}")),
                    });
                }
            }
        }
    }
    bail!("Failed to request latest ISO");
}

// TODO we can validate package names early
#[derive(Clone, Serialize, Deserialize, Validate, Debug, Default)]
pub struct ArchLinuxPackages {
    packages: Vec<String>,
}

impl Prompt for ArchLinuxPackages {
    fn prompt(&mut self, _: &Builder) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fetch_latest_iso() -> Result<()> {
        fetch_latest_iso()?;
        Ok(())
    }
}
