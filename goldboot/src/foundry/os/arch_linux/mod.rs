use super::{BuildImage, DefaultSource};
use crate::cli::prompt::Prompt;
use crate::cli::prompt::PromptNew;
use crate::foundry::Foundry;
use crate::foundry::fabricators::Fabricate;
use crate::foundry::http::HttpServer;
use crate::foundry::options::hostname::Hostname;
use crate::foundry::options::unix_account::RootPassword;
use crate::foundry::os::arch_linux::archinstall::ArchinstallConfig;
use crate::foundry::os::arch_linux::archinstall::ArchinstallCredentials;
use crate::foundry::qemu::{OsCategory, QemuBuilder};
use crate::wait;
use crate::{
    foundry::{FoundryWorker, sources::ImageSource},
    wait_screen_rect,
};
use anyhow::Result;
use anyhow::bail;
use dialoguer::theme::Theme;
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
#[derive(Clone, Serialize, Deserialize, Validate, Debug, Default)]
pub struct ArchLinux {
    #[serde(flatten)]
    pub hostname: Hostname,
    pub mirrorlist: Option<ArchLinuxMirrorlist>,
    #[serde(flatten)]
    pub packages: Option<ArchLinuxPackages>,
    pub root_password: RootPassword,
}

// TODO proc macro
impl Prompt for ArchLinux {
    fn prompt(&mut self, _foundry: &Foundry, _theme: Box<dyn Theme>) -> Result<()> {
        self.root_password = RootPassword::prompt_new(_foundry, _theme)?;
        Ok(())
    }
}

impl DefaultSource for ArchLinux {
    fn default_source(&self, _: ImageArch) -> Result<ImageSource> {
        fetch_latest_iso()
    }
}

impl BuildImage for ArchLinux {
    fn build(&self, worker: &FoundryWorker) -> Result<()> {
        let mut qemu = QemuBuilder::new(&worker, OsCategory::Linux)
            .source(&worker.element.source)?
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
        if let Some(fabricators) = &worker.element.fabricators {
            for fabricator in fabricators {
                fabricator.run(&mut ssh)?;
            }
        }

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
    fn prompt(&mut self, _: &Foundry, _theme: Box<dyn Theme>) -> Result<()> {
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
fn fetch_latest_iso() -> Result<ImageSource> {
    let rs = reqwest::blocking::get(format!(
        "http://mirrors.edge.kernel.org/archlinux/iso/latest/sha256sums.txt"
    ))?;
    if rs.status().is_success() {
        for line in BufReader::new(rs).lines().filter_map(|result| result.ok()) {
            if line.ends_with(".iso") {
                let split: Vec<&str> = line.split_whitespace().collect();
                if let [hash, filename] = split[..] {
                    return Ok(ImageSource::Iso {
                        url: format!(
                            "http://mirrors.edge.kernel.org/archlinux/iso/latest/{filename}"
                        ),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fetch_latest_iso() -> Result<()> {
        fetch_latest_iso()?;
        Ok(())
    }
}
