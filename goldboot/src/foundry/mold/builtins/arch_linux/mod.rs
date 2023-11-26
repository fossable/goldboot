use super::CastImage;
use crate::foundry::mold::options::hostname::Hostname;
use crate::wait;
use crate::{
    enter,
    foundry::{sources::Source, FoundryWorker},
    wait_screen_rect,
};
use goldboot_image::ImageArch;
use log::{debug, info};
use serde::{Deserialize, Serialize};
use simple_error::bail;
use std::{
    error::Error,
    io::{BufRead, BufReader},
};
use validator::Validate;

/// This `Mold` produces an [Arch Linux](https://archlinux.org) image.
#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct ArchLinux {
    pub root_password: Option<RootPassword>,
    pub packages: Option<Packages>,
    pub mirrorlist: Option<Mirrorlist>,
    pub hostname: Option<Hostname>,
}

impl Default for ArchLinux {
    fn default() -> Self {
        Self {
            root_password: RootPassword { plaintext: "root" },
            packages: None,
            mirrorlist: None,
            hostname: Some(Hostname {
                hostname: "ArchLinux".to_string(),
            }),
        }
    }
}

impl CastImage for ArchLinux {
    fn cast(&self, context: &FoundryWorker) -> Result<(), Box<dyn Error>> {
        let mut qemuargs = QemuArgs::new(&context);

        // Start VM
        let mut qemu = qemuargs.start_process()?;

        // Send boot command
        #[rustfmt::skip]
		qemu.vnc.run(vec![
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
        info!("Running base installation");
        match ssh.upload_exec(
            include_bytes!("install.sh"),
            vec![
                ("GB_MIRRORLIST", &self.format_mirrorlist()),
                ("GB_ROOT_PASSWORD", &self.root_password),
            ],
        ) {
            Ok(0) => debug!("Installation completed successfully"),
            _ => bail!("Installation failed"),
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
pub struct Mirrorlist {
    pub mirrors: Vec<String>,
}

//https://archlinux.org/mirrorlist/?country=US&protocol=http&protocol=https&ip_version=4

impl Default for Mirrorlist {
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

impl Prompt for Mirrorlist {
    fn prompt(
        &mut self,
        config: &BuildConfig,
        theme: Box<dyn dialoguer::theme::Theme>,
    ) -> Result<(), Box<dyn Error>> {
        // Prompt mirror list
        {
            let mirror_index = dialoguer::Select::with_theme(&theme)
                .with_prompt("Choose a mirror site")
                .default(0)
                .items(&MIRRORLIST)
                .interact()?;

            self.mirrors = vec![MIRRORLIST[mirror_index].to_string()];
        }

        Ok(())
    }
}

impl Mirrorlist {
    pub fn format_mirrorlist(&self) -> String {
        self.mirrors
            .iter()
            .map(|s| format!("Server = {}", s))
            .collect::<Vec<String>>()
            .join("\n")
    }
}

/// Fetch the latest installation ISO
fn fetch_latest_iso() -> Result<Source, Box<dyn Error>> {
    let rs = reqwest::blocking::get(format!(
        "http://mirror.fossable.org/archlinux/iso/latest/sha1sums.txt"
    ))?;
    if rs.status().is_success() {
        for line in BufReader::new(rs).lines().filter_map(|result| result.ok()) {
            if line.ends_with(".iso") {
                let split: Vec<&str> = line.split_whitespace().collect();
                if let [hash, filename] = split[..] {
                    return Ok(Source::Iso {
                        url: format!("http://mirror.fossable.org/archlinux/iso/latest/{filename}"),
                        checksum: Some(format!("sha1:{hash}")),
                    });
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
    fn test_fetch_latest_iso() -> Result<(), Box<dyn Error>> {
        fetch_latest_iso()?;
        Ok(())
    }
}
