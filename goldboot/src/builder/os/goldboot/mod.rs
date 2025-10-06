use anyhow::{Result, bail};
use goldboot_image::ImageArch;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{
    collections::HashMap,
    io::BufRead,
};
use tracing::debug;
use validator::Validate;

use crate::{
    cli::prompt::Prompt,
    enter,
    builder::{
        Foundry, FoundryWorker,
        http::HttpServer,
        qemu::{OsCategory, QemuBuilder},
        sources::ImageSource,
    },
    input, wait_screen, wait_screen_rect,
};

use super::{
    BuildImage, DefaultSource,
    debian::{DebianEdition, fetch_debian_iso},
};

/// Goldboot Linux is a special-purpose distribution for deploying .gb images.
///
/// Upstream: https://goldboot.org
/// Maintainer: cilki
#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct Goldboot {
    /// Path to the goldboot executable to install. If this isn't given, it
    /// will be downloaded from Github releases.
    pub executable: Option<String>,
}

impl Default for Goldboot {
    fn default() -> Self {
        Self { executable: None }
    }
}

// TODO proc macro
impl Prompt for Goldboot {
    fn prompt(&mut self, _builder: &Foundry) -> Result<()> {
        todo!()
    }
}

impl DefaultSource for Goldboot {
    fn default_source(&self, arch: ImageArch) -> Result<ImageSource> {
        fetch_debian_iso(DebianEdition::default(), arch)
    }
}

impl BuildImage for Goldboot {
    fn build(&self, worker: &FoundryWorker) -> Result<()> {
        // Load goldboot executable
        let exe = if let Some(path) = self.executable.as_ref() {
            std::fs::read(path)?
        } else {
            get_latest_release(OsCategory::Linux, worker.arch)?
        };

        let mut qemu = QemuBuilder::new(&worker, OsCategory::Linux)
            .vga("cirrus")
            .source(&worker.element.source)?
            .drive_files(HashMap::from([("goldboot".to_string(), exe)]))?
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
            wait_screen!("6ee7873098bceb5a2124db82dae6abdae214ce7e"),
			enter!(format!("http://{}:{}/preseed.cfg", http.address, http.port)),
            // Wait for login prompt
            wait_screen!("2eb1ef517849c86a322ba60bb05386decbf00ba5"),
            // Login as root
            enter!("root"),
            enter!("r00tme"),
            // Install goldboot
            enter!("mount /dev/vdb /mnt"),
            enter!("cp /mnt/goldboot /usr/bin/goldboot"),
            enter!("chmod +x /usr/bin/goldboot"),
            // Skip getty login
            enter!("echo 'exec /usr/bin/goldboot' >/root/.xinitrc"),
            enter!("sed -i 's|ExecStart=.*$|ExecStart=/usr/bin/startx|' /usr/lib/systemd/system/getty@.service"),
            // Stop gracefully
            enter!("poweroff"),
		])?;

        qemu.shutdown_wait()?;
        Ok(())
    }
}

/// Download the latest goldboot release.
fn get_latest_release(os: OsCategory, arch: ImageArch) -> Result<Vec<u8>> {
    // List releases
    let releases: Vec<Value> = reqwest::blocking::Client::new()
        .get("https://api.github.com/repos/fossable/goldboot/releases")
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .header("User-Agent", "goldboot")
        .send()?
        .json()?;
    debug!(count = releases.len(), "Total releases");

    // Match the major and minor versions against what we're currently running
    let mut releases: Vec<Map<String, Value>> = releases
        .into_iter()
        .filter_map(|r| match r {
            Value::Object(release) => match release.get("tag_name") {
                Some(Value::String(name)) => {
                    if name.starts_with(&format!(
                        "goldboot-v{}.{}.",
                        crate::built_info::PKG_VERSION_MAJOR,
                        crate::built_info::PKG_VERSION_MINOR
                    )) {
                        Some(release)
                    } else {
                        None
                    }
                }
                _ => None,
            },
            _ => None,
        })
        .collect();

    debug!(count = releases.len(), "Matched releases");

    // Sort by patch version
    releases.sort_by_key(|release| match release.get("tag_name") {
        Some(Value::String(name)) => name.split(".").last().unwrap().parse::<i64>().unwrap(),
        _ => todo!(),
    });

    // Find asset for the given arch
    let asset = match releases.last().unwrap().get("assets") {
        Some(Value::Array(assets)) => assets
            .iter()
            .filter_map(|a| match a {
                Value::Object(asset) => match asset.get("name") {
                    Some(Value::String(name)) => {
                        if name.contains(&arch.as_github_string())
                            && name.contains(&os.as_github_string())
                        {
                            Some(asset)
                        } else {
                            None
                        }
                    }
                    _ => None,
                },
                _ => None,
            })
            .last(),
        _ => None,
    };

    // Download the asset
    if let Some(asset) = asset {
        debug!(asset = ?asset, "Found asset for download");
        match asset.get("browser_download_url") {
            Some(Value::String(url)) => Ok(reqwest::blocking::get(url)?.bytes()?.into()),
            _ => todo!(),
        }
    } else {
        bail!("No release asset found for OS/Arch combination");
    }
}
