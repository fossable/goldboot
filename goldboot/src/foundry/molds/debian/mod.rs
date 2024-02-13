use anyhow::{bail, Result};
use dialoguer::theme::Theme;
use goldboot_image::ImageArch;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader};
use validator::Validate;

use crate::{
    cli::prompt::Prompt,
    enter,
    foundry::{
        http::HttpServer,
        options::{hostname::Hostname, unix_account::RootPassword},
        qemu::QemuBuilder,
        sources::ImageSource,
        Foundry, FoundryWorker,
    },
    input, wait, wait_screen,
};

use super::{CastImage, DefaultSource};

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub enum DebianEdition {
    #[default]
    Bullseye,
    Bookworm,
    Trixie,
    Sid,
}

/// Fetch the latest ISO
pub fn fetch_debian_iso(edition: DebianEdition, arch: ImageArch) -> Result<ImageSource> {
    let arch = match arch {
        ImageArch::Amd64 => "amd64",
        ImageArch::Arm64 => "arm64",
        ImageArch::I386 => "i386",
        _ => bail!("Unsupported architecture"),
    };
    let version = match edition {
        DebianEdition::Bullseye => "11.2.0",
        _ => bail!("Unsupported edition"),
    };

    let rs = reqwest::blocking::get(format!(
        "https://cdimage.debian.org/cdimage/archive/{version}/{arch}/iso-cd/SHA256SUMS"
    ))?;
    if rs.status().is_success() {
        for line in BufReader::new(rs).lines().filter_map(|result| result.ok()) {
            if line.ends_with(".iso") {
                let split: Vec<&str> = line.split_whitespace().collect();
                if let [hash, filename] = split[..] {
                    return Ok(ImageSource::Iso {
						url: format!("https://cdimage.debian.org/cdimage/archive/{version}/{arch}/iso-cd/{filename}"),
						checksum: Some(format!("sha256:{hash}")),
					});
                }
            }
        }
    }
    bail!("Failed to request latest ISO");
}

#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct Debian {
    pub edition: DebianEdition,

    #[serde(flatten)]
    pub hostname: Option<Hostname>,
    pub root_password: Option<RootPassword>,
}

impl Default for Debian {
    fn default() -> Self {
        Self {
            root_password: Some(RootPassword {
                plaintext: "root".to_string(),
            }),
            edition: DebianEdition::default(),
            hostname: Some(Hostname::default()),
        }
    }
}

// TODO proc macro
impl Prompt for Debian {
    fn prompt(&mut self, _foundry: &Foundry, _theme: Box<dyn Theme>) -> Result<()> {
        todo!()
    }
}

impl DefaultSource for Debian {
    fn default_source(&self) -> ImageSource {
        ImageSource::Iso {
            url: "http://mirror.fossable.org/archlinux/iso/2024.01.01/archlinux-2024.01.01-x86_64.iso".to_string(),
            checksum: Some("sha256:12addd7d4154df1caf5f258b80ad72e7a724d33e75e6c2e6adc1475298d47155".to_string()),
        }
    }
}

impl CastImage for Debian {
    fn cast(&self, worker: &FoundryWorker) -> Result<()> {
        let mut qemu = QemuBuilder::new(&worker)
            .source(&worker.element.source)?
            .start()?;

        // Start HTTP
        let http = HttpServer::serve_file(include_bytes!("preseed.cfg"))?;

        // Send boot command
        #[rustfmt::skip]
		qemu.vnc.run(vec![
			wait!(10),
			input!("aa"),
			wait_screen!("53471d73e98f0109ce3262d9c45c522d7574366b"),
			enter!(format!("http://10.0.2.2:{}/preseed.cfg", http.port)),
			wait_screen!("97354165fd270a95fd3da41ef43c35bf24b7c09b"),
			// enter!(&self.root_password),
			// enter!(&self.root_password),
			wait_screen!("33e3bacbff9507e9eb29c73642eaceda12a359c2"),
		])?;

        // Wait for SSH
        let mut ssh = qemu.ssh()?;

        // Shutdown
        ssh.shutdown("poweroff")?;
        qemu.shutdown_wait()?;
        Ok(())
    }
}
