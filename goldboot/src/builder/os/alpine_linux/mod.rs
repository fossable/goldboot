use anyhow::Result;
use goldboot_image::ImageArch;
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;
use std::fmt::Display;
use strum::{Display, EnumIter, IntoEnumIterator};
use validator::Validate;

use crate::{
    builder::{
        Builder,
        options::{hostname::Hostname, iso::Iso, size::Size, unix_account::RootPassword},
        qemu::{OsCategory, QemuBuilder},
    },
    cli::prompt::Prompt,
    enter, wait, wait_screen_rect,
};

use super::BuildImage;

/// Produces [Alpine Linux](https://www.alpinelinux.org) images.
#[derive(Clone, Serialize, Deserialize, Validate, Debug, SmartDefault, goldboot_macros::Prompt)]
pub struct AlpineLinux {
    pub size: Size,
    pub edition: AlpineEdition,
    #[serde(flatten)]
    pub hostname: Hostname,
    pub release: AlpineRelease,
    pub root_password: RootPassword,
    #[default(Iso {
        url: "https://dl-cdn.alpinelinux.org/alpine/v3.19/releases/x86_64/alpine-standard-3.19.1-x86_64.iso".parse().unwrap(),
        checksum: Some("sha256:12addd7d4154df1caf5f258b80ad72e7a724d33e75e6c2e6adc1475298d47155".to_string()),
    })]
    pub iso: Iso,
}

impl BuildImage for AlpineLinux {
    fn build(&self, worker: &Builder) -> Result<()> {
        let mut qemu = QemuBuilder::new(&worker, OsCategory::Linux)
            .with_iso(&self.iso)?
            .prepare_ssh()?
            .start()?;

        // Send boot command
        #[rustfmt::skip]
		qemu.vnc.run(vec![
			// Initial wait
			wait!(30),
			// Root login
			enter!("root"),
			// Configure install
			enter!("export KEYMAPOPTS='us us'"),
			enter!(format!("export HOSTNAMEOPTS='-n {}'", self.hostname.hostname)),
			enter!("export INTERFACESOPTS='
auto lo
iface lo inet loopback

auto eth0
iface eth0 inet dhcp
    hostname alpine-test'"
			),
			enter!("export DNSOPTS='1.1.1.1'"),
			enter!("export TIMEZONEOPTS='-z UTC'"),
			enter!("export PROXYOPTS='none'"),
			enter!("export APKREPOSOPTS='-r'"),
			enter!("export SSHDOPTS='-c openssh'"),
			enter!("export NTPOPTS='-c openntpd'"),
			enter!("export DISKOPTS='-m sys /dev/vda'"),
			// Start install
			enter!("echo -e 'root\nroot\ny' | setup-alpine"),
			wait_screen_rect!("6d7b9fc9229c4f4ae8bc84f0925d8479ccd3e7d2", 668, 0, 1024, 100),
			// Remount root partition
			enter!("mount -t ext4 /dev/vda3 /mnt"),
			// Reboot into installation
			enter!("apk add efibootmgr; efibootmgr -n 0003; reboot"),
		])?;

        // Wait for SSH
        let ssh = qemu.ssh("root")?;

        // Run provisioners
        // TODO

        // Shutdown
        ssh.shutdown("poweroff")?;
        qemu.shutdown_wait()?;
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
    Edge,
    #[serde(rename = "v3.19")]
    V3_19,
    #[serde(rename = "v3.18")]
    V3_18,
    #[serde(rename = "v3.17")]
    V3_17,
    #[serde(rename = "v3.16")]
    V3_16,
    #[serde(rename = "v3.15")]
    V3_15,
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
            match &self {
                AlpineRelease::Edge => "Edge",
                AlpineRelease::V3_19 => "v3.19",
                AlpineRelease::V3_18 => "v3.18",
                AlpineRelease::V3_17 => "v3.17",
                AlpineRelease::V3_16 => "v3.16",
                AlpineRelease::V3_15 => "v3.15",
            }
        )
    }
}

// fn fetch_latest_iso(
//     edition: AlpineEdition,
//     release: AlpineRelease,
//     arch: Architecture,
// ) -> Result<IsoSource> {
//     let arch = match arch {
//         Architecture::amd64 => "x86_64",
//         Architecture::arm64 => "aarch64",
//         _ => bail!("Unsupported architecture"),
//     };

//     let edition = match edition {
//         AlpineEdition::Standard => "standard",
//         AlpineEdition::Extended => "extended",
//         AlpineEdition::Xen => "virt",
//         AlpineEdition::RaspberryPi => "rpi",
//     };

//     let url = format!("https://dl-cdn.alpinelinux.org/alpine/v3.16/releases/{arch}/alpine-{edition}-3.16.0-{arch}.iso");

//     // Download checksum
//     let rs = reqwest::blocking::get(format!("{url}.sha256"))?;
//     let checksum = if rs.status().is_success() { None } else { None };

//     Ok(IsoSource { url, checksum })
// }
