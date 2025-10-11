use anyhow::{Result, bail};
use goldboot_image::ImageArch;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader};
use strum::{Display, EnumIter, IntoEnumIterator};
use validator::Validate;

use crate::{
    builder::{
        Builder,
        http::HttpServer,
        options::{hostname::Hostname, iso::Iso, unix_account::RootPassword},
        qemu::{OsCategory, QemuBuilder},
    },
    cli::prompt::Prompt,
    enter, input, wait_screen, wait_screen_rect,
};

use super::BuildImage;

/// Debian is a Linux distribution composed of free and open-source software and
/// optionally non-free firmware or software developed by the community-supported
/// Debian Project.
///
/// Upstream: https://www.debian.org
/// Maintainer: cilki
#[derive(Clone, Serialize, Deserialize, Validate, Debug, goldboot_macros::Prompt)]
pub struct Debian {
    pub edition: DebianEdition,

    #[serde(flatten)]
    pub hostname: Option<Hostname>,
    pub root_password: RootPassword,

    pub iso: Iso,
}

impl Default for Debian {
    fn default() -> Self {
        Self {
            root_password: RootPassword::default(),
            edition: DebianEdition::default(),
            hostname: Some(Hostname::default()),
            iso: Iso {
                url: "https://example.com".parse().unwrap(),
                checksum: None,
            },
        }
    }
}

impl BuildImage for Debian {
    fn build(&self, worker: &Builder) -> Result<()> {
        let mut qemu = QemuBuilder::new(&worker, OsCategory::Linux)
            .vga("cirrus")
            .with_iso(&self.iso)?
            .prepare_ssh()?
            .start()?;

        // Start HTTP
        let http = HttpServer::new()?
            .file("preseed.cfg", include_bytes!("preseed.cfg"))?
            .serve();

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
			enter!(format!("http://{}:{}/preseed.cfg", http.address, http.port)),
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
        let ssh = qemu.ssh("root")?;

        // Shutdown
        ssh.shutdown("poweroff")?;
        qemu.shutdown_wait()?;
        Ok(())
    }
}

#[derive(Clone, Copy, Serialize, Deserialize, Debug, Default, EnumIter, Display)]
pub enum DebianEdition {
    Bullseye,
    #[default]
    Bookworm,
    Trixie,
    Sid,
}

impl Prompt for DebianEdition {
    fn prompt(&mut self, builder: &Builder) -> Result<()> {
        let editions: Vec<DebianEdition> = DebianEdition::iter().collect();
        let edition_index = dialoguer::Select::with_theme(&crate::cli::cmd::init::theme())
            .with_prompt("Choose Debian edition")
            .default(0)
            .items(editions.iter())
            .interact()?;

        *self = editions[edition_index];
        Ok(())
    }
}

/// Fetch the latest ISO
pub fn fetch_debian_iso(edition: DebianEdition, arch: ImageArch) -> Result<Iso> {
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
                    return Ok(Iso {
                        url: format!(
                            "https://cdimage.debian.org/cdimage/{version}/{arch}/iso-cd/{filename}"
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
