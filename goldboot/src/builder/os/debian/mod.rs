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
    enter, input, wait_screen, wait_screen_rect,
};

use super::BuildImage;

/// Debian is a Linux distribution composed of free and open-source software and
/// optionally non-free firmware or software developed by the community-supported
/// Debian Project.
///
/// Upstream: https://www.debian.org
/// Maintainer: cilki
#[goldboot_macros::Os(architectures(Amd64, Arm64))]
#[derive(Clone, Serialize, Deserialize, Validate, Debug, SmartDefault, goldboot_macros::Prompt)]
pub struct Debian {
    #[default(Arch(ImageArch::Amd64))]
    pub arch: Arch,
    pub size: Size,
    pub edition: DebianEdition,
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

    /// Enable NTP during install
    #[serde(default)]
    pub ntp: Ntp,

    #[default(Iso {
        url: "https://cdimage.debian.org/cdimage/release/current/amd64/iso-cd/debian-13.4.0-amd64-netinst.iso".parse().unwrap(),
        checksum: Some("sha256:0b813535dd76f2ea96eff908c65e8521512c92a0631fd41c95756ffd7d4896dc".to_string()),
    })]
    pub iso: Iso,
}

impl Debian {
    fn generate_preseed(&self) -> String {
        let root_password = match &self.root_password {
            RootPassword::Plaintext(p) => p.clone(),
            RootPassword::PlaintextEnv(name) => {
                std::env::var(name).expect("environment variable not found")
            }
        };

        let extra_packages = {
            let mut pkgs = vec!["openssh-server".to_string()];
            if let Some(packages) = &self.packages {
                pkgs.extend(packages.0.iter().cloned());
            }
            pkgs.join(" ")
        };

        let late_command = self.generate_late_command();

        format!(
            r#"#_preseed_V1
### Localization
d-i debian-installer/locale string {language}.{encoding}
d-i keyboard-configuration/xkb-keymap select {keyboard}

### Network configuration
d-i netcfg/choose_interface select auto
d-i netcfg/get_hostname string {hostname}
d-i netcfg/get_domain string
d-i netcfg/wireless_wep string

### Mirror settings
d-i mirror/country string manual
d-i mirror/http/hostname string http.us.debian.org
d-i mirror/http/directory string /debian
d-i mirror/http/proxy string

### Account setup
d-i passwd/root-login boolean true
d-i passwd/make-user boolean false
d-i passwd/root-password password {root_password}
d-i passwd/root-password-again password {root_password}
d-i passwd/auto-login boolean true

### Clock and time zone setup
d-i clock-setup/utc boolean true
d-i time/zone string {timezone}
d-i clock-setup/ntp boolean {ntp}

### Partitioning
d-i partman-auto/disk string /dev/vda
d-i partman-auto/method string regular
d-i partman-auto-lvm/guided_size string max
d-i partman-lvm/device_remove_lvm boolean true
d-i partman-md/device_remove_md boolean true
d-i partman-lvm/confirm boolean true
d-i partman-lvm/confirm_nooverwrite boolean true
d-i partman-auto/choose_recipe select atomic
d-i partman-partitioning/confirm_write_new_label boolean true
d-i partman/choose_partition select finish
d-i partman/confirm boolean true
d-i partman/confirm_nooverwrite boolean true
d-i partman-efi/non_efi_system boolean true
d-i partman-partitioning/choose_label select gpt
d-i partman-partitioning/default_label string gpt
d-i partman-md/confirm boolean true

### Package selection
tasksel tasksel/first multiselect minimal
d-i pkgsel/include string {extra_packages}
d-i pkgsel/upgrade select safe-upgrade

### Boot loader installation
d-i grub-installer/only_debian boolean true
d-i grub-installer/with_other_os boolean true

### Finishing up
d-i finish-install/reboot_in_progress note
d-i preseed/late_command string {late_command}
"#,
            language = self.locale.language,
            encoding = self.locale.encoding,
            keyboard = self.locale.keyboard,
            hostname = self.hostname.hostname,
            root_password = root_password,
            timezone = self.timezone.0,
            ntp = self.ntp.0,
            extra_packages = extra_packages,
            late_command = late_command,
        )
    }

    fn generate_late_command(&self) -> String {
        let mut cmds: Vec<String> = Vec::new();

        if let Some(users) = &self.users {
            for user in &users.0 {
                cmds.push(format!(
                    "in-target useradd -m -s /bin/bash {}",
                    user.username
                ));
                cmds.push(format!(
                    "in-target sh -c \"echo '{}:{}' | chpasswd\"",
                    user.username, user.password
                ));
                if user.sudo {
                    cmds.push(format!(
                        "in-target usermod -aG sudo {}",
                        user.username
                    ));
                }
            }
        }

        if cmds.is_empty() {
            // preseed requires a non-empty late_command if the key is present,
            // so use a no-op
            "true".to_string()
        } else {
            cmds.join("; ")
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

        // Start HTTP with generated preseed
        let http = HttpServer::new()?
            .file("preseed.cfg", self.generate_preseed().into_bytes())?
            .serve();

        // Send boot command
        #[rustfmt::skip]
		qemu.vnc.run(vec![
            // Wait for boot menu
			wait_screen_rect!("f6852e8b6e072d15270b2b215bbada3da30fd733", 100, 100, 400, 400),
            // Trigger unattended install
			input!("aa"),
            // Wait for preseed URL prompt
            match self.edition {
                DebianEdition::Bullseye => todo!(),
                DebianEdition::Bookworm => wait_screen!("6ee7873098bceb5a2124db82dae6abdae214ce7e"),
                DebianEdition::Trixie   => wait_screen!("6ee7873098bceb5a2124db82dae6abdae214ce7e"),
                DebianEdition::Forky    => todo!(),
                DebianEdition::Sid      => todo!(),
            },
			enter!(format!("http://{}:{}/preseed.cfg", http.address, http.port)),
            // Wait for login prompt
            match self.edition {
                DebianEdition::Bullseye => todo!(),
                DebianEdition::Bookworm => wait_screen!("2eb1ef517849c86a322ba60bb05386decbf00ba5"),
                DebianEdition::Trixie   => wait_screen!("2eb1ef517849c86a322ba60bb05386decbf00ba5"),
                DebianEdition::Forky    => todo!(),
                DebianEdition::Sid      => todo!(),
            },
            // Login as root
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

#[derive(Clone, Copy, Serialize, Deserialize, Debug, Default, EnumIter, Display)]
pub enum DebianEdition {
    /// Trixie — current stable (Debian 13)
    #[default]
    Trixie,
    /// Bookworm — oldstable (Debian 12)
    Bookworm,
    /// Bullseye — oldoldstable (Debian 11)
    Bullseye,
    /// Forky — testing (Debian 14)
    Forky,
    /// Sid — unstable
    Sid,
}

impl Prompt for DebianEdition {
    fn prompt(&mut self, _builder: &Builder) -> Result<()> {
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

/// Fetch the netinst ISO info for a given edition and arch.
pub fn fetch_debian_iso(edition: DebianEdition, arch: ImageArch) -> Result<Iso> {
    let arch_str = match arch {
        ImageArch::Amd64 => "amd64",
        ImageArch::Arm64 => "arm64",
        ImageArch::I386 => "i386",
        _ => bail!("Unsupported architecture"),
    };
    let version_path = match edition {
        DebianEdition::Bullseye => "archive/11.11.0",
        DebianEdition::Bookworm => "archive/12.11.0",
        DebianEdition::Trixie => "release/current",
        _ => bail!("No stable ISO for this edition"),
    };

    let rs = reqwest::blocking::get(format!(
        "https://cdimage.debian.org/cdimage/{version_path}/{arch_str}/iso-cd/SHA256SUMS"
    ))?;
    if rs.status().is_success() {
        for line in BufReader::new(rs).lines().filter_map(|result| result.ok()) {
            if line.ends_with("netinst.iso") {
                let split: Vec<&str> = line.split_whitespace().collect();
                if let [hash, filename] = split[..] {
                    return Ok(Iso {
                        url: format!(
                            "https://cdimage.debian.org/cdimage/{version_path}/{arch_str}/iso-cd/{filename}"
                        )
                        .parse()
                        .unwrap(),
                        checksum: Some(format!("sha256:{hash}")),
                    });
                }
            }
        }
    }
    bail!("Failed to fetch Debian ISO info");
}
