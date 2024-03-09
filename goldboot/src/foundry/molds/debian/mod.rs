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
        qemu::{OsCategory, QemuBuilder},
        sources::ImageSource,
        Foundry, FoundryWorker,
    },
    input, wait, wait_screen, wait_screen_rect,
};

use super::{CastImage, DefaultSource};

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub enum DebianEdition {
    Bullseye,
    #[default]
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
        DebianEdition::Bullseye => "archive/11.9.0",
        DebianEdition::Bookworm => "release/12.5.0",
        _ => bail!("Unsupported edition"),
    };

    let rs = reqwest::blocking::get(format!(
        "https://cdimage.debian.org/cdimage/{version}/{arch}/iso-cd/SHA256SUMS"
    ))?;
    if rs.status().is_success() {
        for line in BufReader::new(rs).lines().filter_map(|result| result.ok()) {
            if line.ends_with(".iso") {
                let split: Vec<&str> = line.split_whitespace().collect();
                if let [hash, filename] = split[..] {
                    return Ok(ImageSource::Iso {
                        url: format!(
                            "https://cdimage.debian.org/cdimage/{version}/{arch}/iso-cd/{filename}"
                        ),
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
    pub root_password: RootPassword,
}

impl Default for Debian {
    fn default() -> Self {
        Self {
            root_password: RootPassword::Plaintext("root".to_string()),
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
    fn default_source(&self, arch: ImageArch) -> Result<ImageSource> {
        fetch_debian_iso(DebianEdition::default(), arch)
    }
}

impl CastImage for Debian {
    fn cast(&self, worker: &FoundryWorker) -> Result<()> {
        let mut qemu = QemuBuilder::new(&worker, OsCategory::Linux)
            .vga("cirrus")
            .source(&worker.element.source)?
            .prepare_ssh()?
            .start()?;

        // Start HTTP
        let http = HttpServer::serve_file(include_bytes!("preseed.cfg"))?;

        // Send boot command
        #[rustfmt::skip]
		qemu.vnc.run(vec![
            // Wait for boot
			wait_screen_rect!("f6852e8b6e072d15270b2b215bbada3da30fd733", 100, 100, 400, 400),
            // Trigger unattended install
			input!("aa"),
            // Wait for preseed URL to be prompted
            match self.edition {
                DebianEdition::Bullseye => todo!(),
                DebianEdition::Bookworm => wait_screen!("6ee7873098bceb5a2124db82dae6abdae214ce7e"),
                DebianEdition::Trixie => todo!(),
                DebianEdition::Sid => todo!(),
            },
			enter!(format!("http://10.0.2.2:{}/preseed.cfg", http.port)),
            // Wait for login prompt
            match self.edition {
                DebianEdition::Bullseye => todo!(),
                DebianEdition::Bookworm => wait_screen!("2eb1ef517849c86a322ba60bb05386decbf00ba5"),
                DebianEdition::Trixie => todo!(),
                DebianEdition::Sid => todo!(),
            },
            // Login as root
            enter!("root"),
            enter!("r00tme"),
		])?;

        // Wait for SSH
        let mut ssh = qemu.ssh("root")?;

        // Shutdown
        ssh.shutdown("poweroff")?;
        qemu.shutdown_wait()?;
        Ok(())
    }
}
