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
            hostname::Hostname,
            iso::Iso,
            locale::Locale,
            ntp::Ntp,
            packages::Packages,
            size::Size,
            timezone::Timezone,
            unix_account::RootPassword,
            unix_users::UnixUsers,
        },
        qemu::{OsCategory, QemuBuilder},
    },
    cli::prompt::Prompt,
    enter, wait, wait_screen_rect,
};

use super::BuildImage;

/// Produces [Alpine Linux](https://www.alpinelinux.org) images.
#[goldboot_macros::Os(architectures(Amd64, Arm64))]
#[derive(Clone, Serialize, Deserialize, Validate, Debug, SmartDefault, goldboot_macros::Prompt)]
pub struct AlpineLinux {
    pub size: Size,
    pub edition: AlpineEdition,
    #[serde(default)]
    pub hostname: Hostname,
    pub release: AlpineRelease,
    #[serde(default)]
    pub root_password: RootPassword,

    /// Additional user accounts to create
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

    /// Disk encryption passphrase (LUKS)
    pub encryption_password: Option<AlpineEncryptionPassword>,

    #[default(Iso {
        url: "https://dl-cdn.alpinelinux.org/alpine/v3.23/releases/x86_64/alpine-standard-3.23.3-x86_64.iso".parse().unwrap(),
        checksum: Some("sha256:966d6bf4d4c79958d43abde84a3e5bbeb4f8c757c164a49d3ec8432be6d36f16".to_string()),
    })]
    pub iso: Iso,
}

impl BuildImage for AlpineLinux {
    fn build(&self, worker: &Builder) -> Result<()> {
        let mut qemu = QemuBuilder::new(&worker, OsCategory::Linux)
            .with_iso(&self.iso)?
            .prepare_ssh()?
            .start()?;

        let ntp_opts = if self.ntp.0 { "-c openntpd" } else { "-c none" };
        let disk_opts = if self.encryption_password.is_some() {
            "-m sys -e /dev/vda"
        } else {
            "-m sys /dev/vda"
        };

        // Send boot command
        #[rustfmt::skip]
		qemu.vnc.run(vec![
			// Initial wait
			wait!(30),
			// Root login
			enter!("root"),
			// Configure install
			enter!(format!("export KEYMAPOPTS='{} {}'", self.locale.keyboard, self.locale.keyboard)),
			enter!(format!("export HOSTNAMEOPTS='-n {}'", self.hostname.hostname)),
			enter!("export INTERFACESOPTS='
auto lo
iface lo inet loopback

auto eth0
iface eth0 inet dhcp
    hostname alpine-test'"
			),
			enter!("export DNSOPTS='1.1.1.1'"),
			enter!(format!("export TIMEZONEOPTS='-z {}'", self.timezone.0)),
			enter!("export PROXYOPTS='none'"),
			enter!("export APKREPOSOPTS='-r'"),
			enter!("export SSHDOPTS='-c openssh'"),
			enter!(format!("export NTPOPTS='{ntp_opts}'")),
			enter!(format!("export DISKOPTS='{disk_opts}'")),
			// Start install
			enter!(format!("echo -e 'root\n{}\ny' | setup-alpine", self.root_password)),
			wait_screen_rect!("6d7b9fc9229c4f4ae8bc84f0925d8479ccd3e7d2", 668, 0, 1024, 100),
			// Remount root partition
			enter!("mount -t ext4 /dev/vda3 /mnt"),
			// Reboot into installation
			enter!("apk add efibootmgr; efibootmgr -n 0003; reboot"),
		])?;

        // Wait for SSH
        let mut ssh = qemu.ssh("root")?;

        // Install extra packages
        if let Some(packages) = &self.packages {
            if !packages.0.is_empty() {
                ssh.exec(&format!("apk add {}", packages.0.join(" ")))?;
            }
        }

        // Create extra users
        if let Some(users) = &self.users {
            for user in &users.0 {
                ssh.exec(&format!("adduser -D {}", user.username))?;
                ssh.exec(&format!(
                    "echo '{}:{}' | chpasswd",
                    user.username, user.password
                ))?;
                if user.sudo {
                    ssh.exec(&format!("addgroup {} wheel", user.username))?;
                }
            }
        }

        // Shutdown
        ssh.shutdown("poweroff")?;
        qemu.shutdown_wait()?;
        Ok(())
    }
}

/// Disk encryption passphrase (LUKS).
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct AlpineEncryptionPassword(pub String);

impl Prompt for AlpineEncryptionPassword {
    fn prompt(&mut self, _: &Builder) -> Result<()> {
        Ok(())
    }
}

#[derive(Clone, Copy, Serialize, Deserialize, Debug, EnumIter, Display, Default)]
pub enum AlpineEdition {
    #[default]
    Standard,
    Extended,
    RaspberryPi,
    Xen,
}

impl Prompt for AlpineEdition {
    fn prompt(&mut self, _: &Builder) -> Result<()> {
        let theme = crate::cli::cmd::init::theme();
        let editions: Vec<AlpineEdition> = AlpineEdition::iter().collect();
        let edition_index = dialoguer::Select::with_theme(&theme)
            .with_prompt("Choose an edition")
            .default(0)
            .items(&editions)
            .interact()?;

        *self = editions[edition_index];
        Ok(())
    }
}

#[derive(Clone, Copy, Serialize, Deserialize, Debug, EnumIter, Default)]
pub enum AlpineRelease {
    #[default]
    #[serde(rename = "v3.23")]
    V3_23,
    #[serde(rename = "v3.22")]
    V3_22,
    #[serde(rename = "v3.21")]
    V3_21,
    #[serde(rename = "v3.20")]
    V3_20,
    #[serde(rename = "v3.19")]
    V3_19,
    Edge,
}

impl Prompt for AlpineRelease {
    fn prompt(&mut self, _: &Builder) -> Result<()> {
        Ok(())
    }
}

impl Display for AlpineRelease {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                AlpineRelease::V3_23 => "v3.23",
                AlpineRelease::V3_22 => "v3.22",
                AlpineRelease::V3_21 => "v3.21",
                AlpineRelease::V3_20 => "v3.20",
                AlpineRelease::V3_19 => "v3.19",
                AlpineRelease::Edge => "edge",
            }
        )
    }
}
