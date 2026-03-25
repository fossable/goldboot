use anyhow::Result;
use goldboot_image::ImageArch;
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;
use strum::{Display, EnumIter, IntoEnumIterator};
use validator::Validate;

use crate::{
    builder::{
        Builder,
        options::{
            arch::Arch, hostname::Hostname, iso::Iso, locale::Locale, ntp::Ntp, packages::Packages,
            size::Size, timezone::Timezone, unix_account::RootPassword, unix_users::UnixUsers,
        },
        qemu::{OsCategory, QemuBuilder},
    },
    cli::prompt::Prompt,
    enter, input, spacebar, tab, wait, wait_screen_rect,
};

use super::BuildImage;

/// Pop!\_OS is a Linux distribution developed by System76, based on Ubuntu.
///
/// Upstream: https://pop.system76.com
/// Maintainer: cilki
#[goldboot_macros::Os(architectures(Amd64))]
#[derive(Clone, Serialize, Deserialize, Validate, Debug, SmartDefault, goldboot_macros::Prompt)]
pub struct PopOs {
    #[default(Arch(ImageArch::Amd64))]
    pub arch: Arch,
    pub size: Size,
    pub release: PopOsRelease,
    pub edition: PopOsEdition,
    #[serde(default)]
    pub hostname: Hostname,
    #[serde(default)]
    pub root_password: RootPassword,

    /// Primary user account (required by the Pop!_OS installer)
    #[serde(default)]
    pub user: PopOsUser,

    /// Additional user accounts to create post-install
    pub users: Option<UnixUsers>,

    /// Packages to install
    pub packages: Option<Packages>,

    /// System timezone
    #[serde(default)]
    pub timezone: Timezone,

    /// Locale and keyboard settings
    #[serde(default)]
    pub locale: Locale,

    /// Enable NTP time synchronization
    #[serde(default)]
    pub ntp: Ntp,

    #[default(Iso {
        url: "https://iso.pop-os.org/24.04/amd64/generic/23/pop-os_24.04_amd64_generic_23.iso".parse().unwrap(),
        checksum: Some("sha256:7eb7c1a21674d0bd7d51a95b159bea25df5f97da8f2f7cd32c58dfc17746f70d".to_string()),
    })]
    pub iso: Iso,
}

impl BuildImage for PopOs {
    fn build(&self, worker: &Builder) -> Result<()> {
        let mut qemu = QemuBuilder::new(&worker, OsCategory::Linux)
            .with_iso(&self.iso)?
            .prepare_ssh()?
            .start()?;

        let root_password = match &self.root_password {
            RootPassword::Plaintext(p) => p.clone(),
            RootPassword::PlaintextEnv(name) => {
                std::env::var(name).expect("environment variable not found")
            }
        };

        // Send boot command — drives the Pop!_OS graphical installer via VNC
        #[rustfmt::skip]
        qemu.vnc.run(vec![
            // Wait for live environment to boot
            wait!(120),
            // Select language: English
            enter!(),
            // Select location: United States
            enter!(),
            // Select keyboard layout
            enter!(),
            enter!(),
            // Select clean install
            spacebar!(),
            enter!(),
            // Select disk (/dev/vda)
            spacebar!(),
            enter!(),
            // Configure username
            enter!(self.user.username),
            // Configure password
            input!(self.user.password),
            tab!(),
            enter!(self.user.password),
            // Skip disk encryption
            enter!(),
            // Wait for installation
            wait_screen_rect!("TODO", 100, 0, 1024, 200),
            // Reboot
            enter!(),
            wait!(60),
            // Login as primary user
            enter!(self.user.password),
            wait!(30),
            // Open terminal and escalate to root
            enter!("sudo -i"),
            enter!(self.user.password),
            // Set root password
            enter!("passwd"),
            enter!(root_password),
            enter!(root_password),
            // Set hostname
            enter!(format!("hostnamectl set-hostname {}", self.hostname.hostname)),
            // Set timezone
            enter!(format!("timedatectl set-timezone {}", self.timezone.0)),
            // Enable/disable NTP
            enter!(format!("timedatectl set-ntp {}", self.ntp.0)),
            // Enable SSH
            enter!("apt install -y openssh-server"),
            enter!("echo 'PermitRootLogin yes' >>/etc/ssh/sshd_config"),
            enter!("systemctl restart sshd"),
        ])?;

        // Wait for SSH
        let mut ssh = qemu.ssh("root")?;

        // Install extra packages
        if let Some(packages) = &self.packages {
            if !packages.0.is_empty() {
                ssh.exec(&format!("apt install -y {}", packages.0.join(" ")))?;
            }
        }

        // Create extra users
        if let Some(users) = &self.users {
            for user in &users.0 {
                ssh.exec(&format!("useradd -m -s /bin/bash {}", user.username))?;
                ssh.exec(&format!(
                    "echo '{}:{}' | chpasswd",
                    user.username, user.password
                ))?;
                if user.sudo {
                    ssh.exec(&format!("usermod -aG sudo {}", user.username))?;
                }
            }
        }

        // Shutdown
        ssh.shutdown("poweroff")?;
        qemu.shutdown_wait()?;
        Ok(())
    }
}

/// The primary user account created by the Pop!_OS installer.
#[derive(Clone, Serialize, Deserialize, Validate, Debug, SmartDefault)]
pub struct PopOsUser {
    #[default("user".to_string())]
    pub username: String,
    #[default("changeme".to_string())]
    pub password: String,
}

impl Prompt for PopOsUser {
    fn prompt(&mut self, _: &Builder) -> Result<()> {
        Ok(())
    }
}

#[derive(Clone, Copy, Serialize, Deserialize, Debug, Default, EnumIter, Display)]
pub enum PopOsEdition {
    /// Generic (Intel/AMD graphics)
    #[default]
    Generic,
    /// NVIDIA proprietary drivers
    Nvidia,
}

impl Prompt for PopOsEdition {
    fn prompt(&mut self, _: &Builder) -> Result<()> {
        let editions: Vec<PopOsEdition> = PopOsEdition::iter().collect();
        let index = dialoguer::Select::with_theme(&crate::cli::cmd::init::theme())
            .with_prompt("Choose Pop!_OS edition")
            .default(0)
            .items(&editions)
            .interact()?;
        *self = editions[index];
        Ok(())
    }
}

#[derive(Clone, Copy, Serialize, Deserialize, Debug, EnumIter, Default)]
pub enum PopOsRelease {
    /// 24.04 LTS (current)
    #[default]
    #[serde(rename = "24.04")]
    V24_04,
    /// 22.04 LTS
    #[serde(rename = "22.04")]
    V22_04,
}

impl std::fmt::Display for PopOsRelease {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                PopOsRelease::V24_04 => "24.04 LTS",
                PopOsRelease::V22_04 => "22.04 LTS",
            }
        )
    }
}

impl Prompt for PopOsRelease {
    fn prompt(&mut self, _: &Builder) -> Result<()> {
        let releases: Vec<PopOsRelease> = PopOsRelease::iter().collect();
        let index = dialoguer::Select::with_theme(&crate::cli::cmd::init::theme())
            .with_prompt("Choose Pop!_OS release")
            .default(0)
            .items(&releases)
            .interact()?;
        *self = releases[index];
        Ok(())
    }
}
