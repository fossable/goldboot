use anyhow::Result;
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;
use std::fmt::Display;
use strum::{Display, EnumIter, IntoEnumIterator};
use validator::Validate;

use crate::{
    builder::{
        Builder,
        options::{
            hostname::Hostname, iso::Iso, packages::Packages, size::Size,
            unix_account::RootPassword, unix_users::UnixUsers,
        },
        qemu::{OsCategory, QemuBuilder},
    },
    cli::prompt::Prompt,
    enter, wait, wait_screen_rect,
};

use super::BuildImage;

/// Produces [Tiny Core Linux](http://www.tinycorelinux.net) images.
///
/// Uses the `CorePure64` edition by default (headless, ~26 MB ISO).
/// Installation is performed via `tc-install` over VNC.
#[goldboot_macros::Os(architectures(Amd64))]
#[derive(Clone, Serialize, Deserialize, Validate, Debug, SmartDefault, goldboot_macros::Prompt)]
pub struct TinyCore {
    pub size: Size,
    pub edition: TinyCoreEdition,
    pub release: TinyCoreRelease,
    #[serde(default)]
    pub hostname: Hostname,
    #[serde(default)]
    pub root_password: RootPassword,

    /// Additional user accounts to create
    pub users: Option<UnixUsers>,

    /// Packages (tce extensions) to install
    pub packages: Option<Packages>,

    #[default(Iso {
        url: "http://www.tinycorelinux.net/17.x/x86_64/release/CorePure64-17.0.iso".parse().unwrap(),
        checksum: None,
    })]
    pub iso: Iso,
}

impl BuildImage for TinyCore {
    fn build(&self, worker: &Builder) -> Result<()> {
        let mut qemu = QemuBuilder::new(&worker, OsCategory::Linux)
            .with_iso(&self.iso)?
            .prepare_ssh()?
            .start()?;

        // tc-install arguments:
        //   -f : frugal install (recommended for disk installs)
        //   -z : install to whole disk (no separate boot partition prompt)
        //   -d /dev/vda : target disk
        //   -s <size>   : swap size in MB (0 = no swap)
        //   -b          : install bootloader
        //   -r          : remaster (copy current running extensions)
        #[rustfmt::skip]
        qemu.vnc.run(vec![
            // Wait for boot to complete
            wait!(30),
            // Run tc-install for a frugal sys install onto /dev/vda
            enter!("sudo tc-install -f -z -d /dev/vda -s 0 -b"),
            // tc-install prompts: confirm install
            wait_screen_rect!("tc_install_confirm", 0, 0, 1024, 768),
            enter!("y"),
            // Wait for install to finish and reboot prompt
            wait_screen_rect!("tc_install_done", 0, 0, 1024, 768),
            enter!("sudo reboot"),
        ])?;

        // Wait for SSH after reboot
        let mut ssh = qemu.ssh("tc")?;

        // Set root password
        ssh.exec(&format!(
            "echo 'root:{}' | sudo chpasswd",
            self.root_password
        ))?;

        // Set hostname
        ssh.exec(&format!(
            "echo '{}' | sudo tee /etc/hostname",
            self.hostname.hostname
        ))?;

        // Install tce extensions
        if let Some(packages) = &self.packages {
            for pkg in &packages.0 {
                ssh.exec(&format!("tce-load -wi {pkg}"))?;
            }
        }

        // Create extra users
        if let Some(users) = &self.users {
            for user in &users.0 {
                ssh.exec(&format!("sudo adduser -D {}", user.username))?;
                ssh.exec(&format!(
                    "echo '{}:{}' | sudo chpasswd",
                    user.username, user.password
                ))?;
                if user.sudo {
                    ssh.exec(&format!("sudo addgroup {} wheel", user.username))?;
                }
            }
        }

        // Shutdown
        ssh.shutdown("sudo poweroff")?;
        qemu.shutdown_wait()?;
        Ok(())
    }
}

#[derive(Clone, Copy, Serialize, Deserialize, Debug, EnumIter, Display, Default)]
pub enum TinyCoreEdition {
    /// Minimal 64-bit edition (~26 MB), command-line only
    #[default]
    CorePure64,
    /// 64-bit edition with FLTK/FLWM desktop (~43 MB)
    TinyCorePure64,
}

impl Prompt for TinyCoreEdition {
    fn prompt(&mut self, _: &Builder) -> Result<()> {
        let theme = crate::cli::cmd::init::theme();
        let editions: Vec<TinyCoreEdition> = TinyCoreEdition::iter().collect();
        let idx = dialoguer::Select::with_theme(&theme)
            .with_prompt("Choose an edition")
            .default(0)
            .items(&editions)
            .interact()?;
        *self = editions[idx];
        Ok(())
    }
}

#[derive(Clone, Copy, Serialize, Deserialize, Debug, EnumIter, Default)]
pub enum TinyCoreRelease {
    #[default]
    #[serde(rename = "17.0")]
    V17_0,
}

impl Display for TinyCoreRelease {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                TinyCoreRelease::V17_0 => "17.0",
            }
        )
    }
}

impl Prompt for TinyCoreRelease {
    fn prompt(&mut self, _: &Builder) -> Result<()> {
        Ok(())
    }
}
