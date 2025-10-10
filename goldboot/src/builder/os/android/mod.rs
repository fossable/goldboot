use super::{CastImage, DefaultSource};
use crate::builder::Foundry;
use crate::builder::options::hostname::Hostname;
use crate::builder::options::unix_account::RootPassword;
use crate::builder::qemu::QemuBuilder;
use crate::cli::prompt::Prompt;
use crate::wait;
use crate::{
    builder::{Builder, sources::ImageSource},
    wait_screen_rect,
};
use anyhow::Result;
use anyhow::bail;
use dialoguer::theme::Theme;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader};
use tracing::{debug, info};
use validator::Validate;

/// Produces an AOSP image.
#[derive(Clone, Serialize, Deserialize, Validate, Debug, goldboot_macros::Prompt)]
pub struct Android {}

impl Default for Android {
    fn default() -> Self {
        Self {}
    }
}

impl DefaultSource for Android {
    fn default_source(&self, arch: ImageArch) -> Result<ImageSource> {
        todo!()
    }
}

impl CastImage for Android {
    fn cast(&self, worker: &Builder) -> Result<()> {
        let mut qemu = QemuBuilder::new(&worker).start()?;

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
        let mut ssh = qemu.ssh()?;

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
