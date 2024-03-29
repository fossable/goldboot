use anyhow::Result;
use dialoguer::theme::Theme;
use goldboot_image::ImageArch;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use strum::{Display, EnumIter, IntoEnumIterator};
use validator::Validate;

use crate::{
    cli::prompt::{Prompt, PromptNew},
    enter,
    foundry::{
        options::{hostname::Hostname, unix_account::RootPassword},
        qemu::{OsCategory, QemuBuilder},
        sources::ImageSource,
        Foundry, FoundryWorker,
    },
    wait, wait_screen_rect,
};

use super::{CastImage, DefaultSource};

/// Produces [Alpine Linux](https://www.alpinelinux.org) images.
#[derive(Clone, Serialize, Deserialize, Validate, Debug, Default)]
pub struct AlpineLinux {
    pub edition: AlpineEdition,
    #[serde(flatten)]
    pub hostname: Hostname,
    pub release: AlpineRelease,
    pub root_password: RootPassword,
}

impl DefaultSource for AlpineLinux {
    fn default_source(&self, _: ImageArch) -> Result<ImageSource> {
        Ok(ImageSource::Iso {
            url: "https://dl-cdn.alpinelinux.org/alpine/v3.19/releases/x86_64/alpine-standard-3.19.1-x86_64.iso".to_string(),
            checksum: Some("sha256:12addd7d4154df1caf5f258b80ad72e7a724d33e75e6c2e6adc1475298d47155".to_string()),
        })
    }
}

// TODO proc macro
impl Prompt for AlpineLinux {
    fn prompt(&mut self, _foundry: &Foundry, _theme: Box<dyn Theme>) -> Result<()> {
        self.root_password = RootPassword::prompt_new(_foundry, _theme)?;
        Ok(())
    }
}

impl CastImage for AlpineLinux {
    fn cast(&self, worker: &FoundryWorker) -> Result<()> {
        let mut qemu = QemuBuilder::new(&worker, OsCategory::Linux)
            .source(&worker.element.source)?
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
        let mut ssh = qemu.ssh("root")?;

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

impl PromptNew for AlpineEdition {
    fn prompt_new(
        foundry: &crate::foundry::Foundry,
        theme: Box<dyn dialoguer::theme::Theme>,
    ) -> Result<Self> {
        let editions: Vec<AlpineEdition> = AlpineEdition::iter().collect();
        let edition_index = dialoguer::Select::with_theme(&*theme)
            .with_prompt("Choose an edition")
            .default(0)
            .items(&editions)
            .interact()?;

        Ok(editions[edition_index])
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
