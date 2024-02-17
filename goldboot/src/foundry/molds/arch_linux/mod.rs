use super::{CastImage, DefaultSource};
use crate::cli::prompt::Prompt;
use crate::foundry::options::hostname::Hostname;
use crate::foundry::options::unix_account::RootPassword;
use crate::foundry::qemu::{OsCategory, QemuBuilder};
use crate::foundry::sources::iso::IsoSource;
use crate::foundry::Foundry;
use crate::wait;
use crate::{
    foundry::{sources::ImageSource, FoundryWorker},
    wait_screen_rect,
};
use anyhow::bail;
use anyhow::Result;
use dialoguer::theme::Theme;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader};
use tracing::{debug, info};
use validator::Validate;

/// This `Mold` produces an [Arch Linux](https://archlinux.org) image.
#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct ArchLinux {
    #[serde(flatten)]
    pub hostname: Option<Hostname>,
    pub mirrorlist: Option<ArchLinuxMirrorlist>,
    pub packages: Option<ArchLinuxPackages>,
    pub root_password: Option<RootPassword>,
}

impl Default for ArchLinux {
    fn default() -> Self {
        Self {
            root_password: Some(RootPassword {
                plaintext: "root".to_string(),
            }),
            packages: None,
            mirrorlist: None,
            hostname: Some(Hostname {
                hostname: "ArchLinux".to_string(),
            }),
        }
    }
}

// TODO proc macro
impl Prompt for ArchLinux {
    fn prompt(&mut self, _foundry: &Foundry, _theme: Box<dyn Theme>) -> Result<()> {
        todo!()
    }
}

impl DefaultSource for ArchLinux {
    fn default_source(&self) -> ImageSource {
        ImageSource::Iso {
            url: "http://mirror.fossable.org/archlinux/iso/2024.01.01/archlinux-2024.01.01-x86_64.iso".to_string(),
            checksum: Some("sha256:12addd7d4154df1caf5f258b80ad72e7a724d33e75e6c2e6adc1475298d47155".to_string()),
        }
    }
}

impl CastImage for ArchLinux {
    fn cast(&self, worker: &FoundryWorker) -> Result<()> {
        let mut qemu = QemuBuilder::new(&worker, OsCategory::Linux)
            .source(&worker.element.source)?
            .start()?;

        // Send boot command
        #[rustfmt::skip]
		qemu.vnc.run(vec![
			// Initial wait
			wait!(30),
			// Wait for login
			wait_screen_rect!("5b3ca88689e9d671903b3040889c7fa1cb5f244a", 100, 0, 1024, 400),
			// Configure root password
			// enter!("passwd"), enter!(self.root_password), enter!(self.root_password),
		])?;

        // Wait for SSH
        let mut ssh = qemu.ssh("root")?;

        // Run install script
        info!("Running base installation");
        match ssh.upload_exec(
            include_bytes!("install.sh"),
            vec![
                // ("GB_MIRRORLIST", &self.format_mirrorlist()),
                // ("GB_ROOT_PASSWORD", &self.root_password),
            ],
        ) {
            Ok(0) => debug!("Installation completed successfully"),
            _ => bail!("Installation failed"),
        }

        // Run provisioners
        // self.provisioners.run(&mut ssh)?;

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
        "http://mirror.fossable.org/archlinux/iso/latest/sha256sums.txt"
    ))?;
    if rs.status().is_success() {
        for line in BufReader::new(rs).lines().filter_map(|result| result.ok()) {
            if line.ends_with(".iso") {
                let split: Vec<&str> = line.split_whitespace().collect();
                if let [hash, filename] = split[..] {
                    return Ok(ImageSource::Iso {
                        url: format!("http://mirror.fossable.org/archlinux/iso/latest/{filename}"),
                        checksum: Some(format!("sha256:{hash}")),
                    });
                }
            }
        }
    }
    bail!("Failed to request latest ISO");
}

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
