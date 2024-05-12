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

use super::{
    debian::{fetch_debian_iso, DebianEdition},
    CastImage, DefaultSource,
};

#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct Goldboot {}

impl Default for Goldboot {
    fn default() -> Self {
        Self {}
    }
}

// TODO proc macro
impl Prompt for Goldboot {
    fn prompt(&mut self, _foundry: &Foundry, _theme: Box<dyn Theme>) -> Result<()> {
        todo!()
    }
}

impl DefaultSource for Goldboot {
    fn default_source(&self, arch: ImageArch) -> Result<ImageSource> {
        fetch_debian_iso(DebianEdition::default(), arch)
    }
}

impl CastImage for Goldboot {
    fn cast(&self, worker: &FoundryWorker) -> Result<()> {
        let mut qemu = QemuBuilder::new(&worker, OsCategory::Linux)
            .vga("cirrus")
            .source(&worker.element.source)?
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
            wait_screen!("6ee7873098bceb5a2124db82dae6abdae214ce7e"),
			enter!(format!("http://{}:{}/preseed.cfg", http.address, http.port)),
            // Wait for login prompt
            wait_screen!("2eb1ef517849c86a322ba60bb05386decbf00ba5"),
            // Login as root
            enter!("root"),
            enter!("r00tme"),
		])?;

        // Wait for SSH
        let mut ssh = qemu.ssh("root")?;

        // Install executable
        // ssh.upload(
        //     std::fs::read(&self.executable)?,
        //     "/mnt/usr/bin/goldboot-linux",
        // )?;

        // Shutdown
        ssh.shutdown("poweroff")?;
        qemu.shutdown_wait()?;
        Ok(())
    }
}
