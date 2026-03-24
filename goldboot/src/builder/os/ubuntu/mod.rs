use anyhow::{Result, bail};
use goldboot_image::ImageArch;
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;
use std::io::{BufRead, BufReader};
use strum::{Display, EnumIter, IntoEnumIterator};
use validator::Validate;

use crate::{
    builder::{
        Builder,
        http::HttpServer,
        options::{
            arch::Arch,
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
    enter, wait_screen_rect,
};

use super::BuildImage;

/// Ubuntu is a Linux distribution derived from Debian and composed mostly of
/// free and open-source software.
///
/// Upstream: https://ubuntu.com
/// Maintainer: cilki
#[goldboot_macros::Os(architectures(Amd64, Arm64))]
#[derive(Clone, Serialize, Deserialize, Validate, Debug, SmartDefault, goldboot_macros::Prompt)]
pub struct Ubuntu {
    #[default(Arch(ImageArch::Amd64))]
    pub arch: Arch,
    pub size: Size,
    pub release: UbuntuRelease,
    #[serde(default)]
    pub hostname: Hostname,
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

    #[default(Iso {
        url: "https://releases.ubuntu.com/noble/ubuntu-24.04.4-live-server-amd64.iso".parse().unwrap(),
        checksum: Some("sha256:e907d92eeec9df64163a7e454cbc8d7755e8ddc7ed42f99dbc80c40f1a138433".to_string()),
    })]
    pub iso: Iso,
}

impl Ubuntu {
    /// Generate an autoinstall user-data YAML document.
    fn generate_autoinstall(&self) -> String {
        let root_password = match &self.root_password {
            RootPassword::Plaintext(p) => p.clone(),
            RootPassword::PlaintextEnv(name) => {
                std::env::var(name).expect("environment variable not found")
            }
        };

        let extra_packages: Vec<String> = self
            .packages
            .as_ref()
            .map(|p| p.0.clone())
            .unwrap_or_default();

        let packages_yaml = if extra_packages.is_empty() {
            String::new()
        } else {
            format!(
                "packages:\n{}\n",
                extra_packages
                    .iter()
                    .map(|p| format!("  - {p}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        };

        let extra_users_yaml = self
            .users
            .as_ref()
            .map(|users| {
                users
                    .0
                    .iter()
                    .map(|u| {
                        let groups = if u.sudo {
                            "    groups: [sudo]\n"
                        } else {
                            ""
                        };
                        format!(
                            "  - name: {}\n    passwd: {}\n    lock_passwd: false\n{}",
                            u.username, u.password, groups
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_default();

        let users_section = format!(
            "users:\n  - name: root\n    passwd: {root_password}\n    lock_passwd: false\n{extra_users_yaml}"
        );

        format!(
            r#"#cloud-config
autoinstall:
  version: 1
  locale: {language}.{encoding}
  keyboard:
    layout: {keyboard}
  network:
    network:
      version: 2
      ethernets:
        any:
          match:
            name: "en*"
          dhcp4: true
  storage:
    layout:
      name: direct
  {packages_yaml}identity:
    hostname: {hostname}
    username: root
    password: {root_password}
  {users_section}
  ntp:
    enabled: {ntp}
  timezone: {timezone}
  ssh:
    install-server: true
    allow-pw: true
  late-commands: []
"#,
            language = self.locale.language,
            encoding = self.locale.encoding,
            keyboard = self.locale.keyboard,
            hostname = self.hostname.hostname,
            root_password = root_password,
            ntp = self.ntp.0,
            timezone = self.timezone.0,
            packages_yaml = packages_yaml,
            users_section = users_section,
        )
    }
}

impl BuildImage for Ubuntu {
    fn build(&self, worker: &Builder) -> Result<()> {
        let mut qemu = QemuBuilder::new(&worker, OsCategory::Linux)
            .with_iso(&self.iso)?
            .prepare_ssh()?
            .start()?;

        // Serve autoinstall config via HTTP
        let http = HttpServer::new()?
            .file("user-data", self.generate_autoinstall().into_bytes())?
            .file("meta-data", b"".to_vec())?
            .serve();

        // Send boot command — at the GRUB menu, append autoinstall ds= kernel param
        #[rustfmt::skip]
        qemu.vnc.run(vec![
            // Wait for GRUB menu
            wait_screen_rect!("TODO", 100, 0, 1024, 200),
            // Select "Try or Install Ubuntu Server", append kernel params
            enter!(format!(
                " autoinstall ds=nocloud;s=http://{}:{}/",
                http.address, http.port
            )),
            // Wait for install to complete and reach login
            wait_screen_rect!("TODO", 100, 0, 1024, 200),
            enter!("root"),
            enter!(match &self.root_password {
                RootPassword::Plaintext(p) => p.clone(),
                RootPassword::PlaintextEnv(name) => std::env::var(name).expect("environment variable not found"),
            }),
        ])?;

        // Wait for SSH
        let ssh = qemu.ssh("root")?;

        // Shutdown
        ssh.shutdown("poweroff")?;
        qemu.shutdown_wait()?;
        Ok(())
    }
}

#[derive(Clone, Copy, Serialize, Deserialize, Debug, EnumIter, Display)]
pub enum UbuntuRelease {
    /// 25.10 (Questing Quokka) — interim
    #[serde(rename = "25.10")]
    V25_10,
    /// 24.04 LTS (Noble Numbat)
    #[serde(rename = "24.04")]
    V24_04,
    /// 22.04 LTS (Jammy Jellyfish)
    #[serde(rename = "22.04")]
    V22_04,
}

impl Default for UbuntuRelease {
    fn default() -> Self {
        Self::V24_04
    }
}

impl UbuntuRelease {
    pub fn codename(&self) -> &'static str {
        match self {
            UbuntuRelease::V25_10 => "questing",
            UbuntuRelease::V24_04 => "noble",
            UbuntuRelease::V22_04 => "jammy",
        }
    }
}

impl Prompt for UbuntuRelease {
    fn prompt(&mut self, _: &Builder) -> Result<()> {
        let releases: Vec<UbuntuRelease> = UbuntuRelease::iter().collect();
        let index = dialoguer::Select::with_theme(&crate::cli::cmd::init::theme())
            .with_prompt("Choose Ubuntu release")
            .default(0)
            .items(&releases)
            .interact()?;
        *self = releases[index];
        Ok(())
    }
}

/// Fetch live-server ISO info for a given release and arch.
pub fn fetch_ubuntu_iso(release: UbuntuRelease, arch: ImageArch) -> Result<Iso> {
    let arch_str = match arch {
        ImageArch::Amd64 => "amd64",
        ImageArch::Arm64 => "arm64",
        _ => bail!("Unsupported architecture"),
    };
    let codename = release.codename();

    let rs = reqwest::blocking::get(format!(
        "https://releases.ubuntu.com/{codename}/SHA256SUMS"
    ))?;
    if rs.status().is_success() {
        for line in BufReader::new(rs).lines().filter_map(|result| result.ok()) {
            if line.contains("live-server") && line.contains(arch_str) && line.ends_with(".iso") {
                let split: Vec<&str> = line.split_whitespace().collect();
                if let [hash, filename] = split[..] {
                    // SHA256SUMS uses " *filename" format — strip leading '*'
                    let filename = filename.trim_start_matches('*');
                    return Ok(Iso {
                        url: format!("https://releases.ubuntu.com/{codename}/{filename}")
                            .parse()
                            .unwrap(),
                        checksum: Some(format!("sha256:{hash}")),
                    });
                }
            }
        }
    }
    bail!("Failed to fetch Ubuntu ISO info for {codename}/{arch_str}");
}
