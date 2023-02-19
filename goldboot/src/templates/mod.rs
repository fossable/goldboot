use crate::{build::BuildWorker, *};
use serde::{Deserialize, Serialize};
use std::{error::Error, fmt::Display, path::Path};

pub mod linux;

// Temporary
//pub mod macos;
//pub mod windows;

pub trait BuildTemplate {
    /// Build an image from the template.
    fn build(&self, context: &BuildWorker) -> Result<(), Box<dyn Error>>;
}

/// Represents a "base configuration" that users can modify and use to build
/// images.
#[derive(Clone, Serialize, Deserialize, Debug, EnumIter)]
#[serde(tag = "type")]
pub enum Template {
    Alpine(templates::linux::alpine::AlpineTemplate),
    Arch,
    Artix,
    Bedrock,
    CentOs,
    Debian,
    ElementaryOs,
    Fedora,
    FreeBsd,
    Gentoo,
    Goldboot,
    Haiku,
    Kali,
    Mint,
    MacOs,
    Manjaro,
    NetBsd,
    NixOs,
    OpenBsd,
    OpenSuse,
    Oracle,
    Parrot,
    PopOs,
    Qubes,
    RedHat,
    Rocky,
    Slackware,
    SteamDeck,
    SteamOs,
    Tails,
    TrueNas,
    Ubuntu,
    Void,
    Windows10,
    Windows11,
    Windows7,
    Zorin,
}

impl Display for Template {
    #[rustfmt::skip]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(
			f,
			"{}",
			match &self {
				Template::Alpine       => "Alpine Linux",
				Template::Arch         => "Arch Linux",
				Template::Artix        => "Artix Linux",
				Template::Bedrock      => "Bedrock Linux",
				Template::CentOs       => "CentOS",
				Template::Debian       => "Debian",
				Template::ElementaryOs => "ElementaryOS",
				Template::Fedora       => "Fedora",
				Template::FreeBsd      => "FreeBSD",
				Template::Gentoo       => "Gentoo Linux",
				Template::Goldboot     => "goldboot Linux",
				Template::Haiku        => "Haiku",
				Template::Kali         => "Kali Linux",
				Template::Mint         => "Linux Mint",
				Template::MacOs        => "macOS",
				Template::Manjaro      => "Manjaro",
				Template::NetBsd       => "NetBSD",
				Template::NixOs        => "Nix OS",
				Template::OpenBsd      => "OpenBSD",
				Template::OpenSuse     => "OpenSUSE",
				Template::Oracle       => "Oracle Linux",
				Template::Parrot       => "Parrot Linux",
				Template::PopOs        => "Pop_OS!",
				Template::Qubes        => "Qubes Linux",
				Template::RedHat       => "RedHat Enterprise Linux",
				Template::Rocky        => "Rocky Linux",
				Template::Slackware    => "Slackware",
				Template::SteamDeck    => "Steam Deck",
				Template::SteamOs      => "Steam OS",
				Template::Tails        => "Tails Linux",
				Template::TrueNas      => "TrueNAS Core",
				Template::Ubuntu       => "Ubuntu",
				Template::Void         => "Void Linux",
				Template::Windows10    => "Microsoft Windows 10",
				Template::Windows11    => "Microsoft Windows 11",
				Template::Windows7     => "Microsoft Windows 7",
				Template::Zorin        => "Zorin OS",
			}
		)
	}
}

impl Template {
    pub fn architectures(&self) -> Vec<Architecture> {
        match &self {
            TemplateId::Alpine => vec![Architecture::amd64, Architecture::i386],
            TemplateId::Arch => vec![Architecture::amd64, Architecture::i386],
            TemplateId::Artix => vec![Architecture::amd64],
            TemplateId::Bedrock => vec![Architecture::amd64],
            TemplateId::CentOs => vec![Architecture::amd64],
            TemplateId::Debian => vec![Architecture::amd64],
            TemplateId::ElementaryOs => vec![Architecture::amd64],
            TemplateId::Fedora => vec![Architecture::amd64],
            TemplateId::FreeBsd => vec![Architecture::amd64],
            TemplateId::Gentoo => vec![Architecture::amd64],
            TemplateId::Goldboot => vec![Architecture::amd64],
            TemplateId::Haiku => vec![Architecture::amd64],
            TemplateId::Kali => vec![Architecture::amd64],
            TemplateId::Mint => vec![Architecture::amd64],
            TemplateId::MacOs => vec![Architecture::amd64],
            TemplateId::Manjaro => vec![Architecture::amd64],
            TemplateId::NetBsd => vec![Architecture::amd64],
            TemplateId::NixOs => vec![Architecture::amd64],
            TemplateId::OpenBsd => vec![Architecture::amd64],
            TemplateId::OpenSuse => vec![Architecture::amd64],
            TemplateId::Oracle => vec![Architecture::amd64],
            TemplateId::Parrot => vec![Architecture::amd64],
            TemplateId::PopOs => vec![Architecture::amd64],
            TemplateId::Qubes => vec![Architecture::amd64],
            TemplateId::RedHat => vec![Architecture::amd64],
            TemplateId::Rocky => vec![Architecture::amd64],
            TemplateId::Slackware => vec![Architecture::amd64],
            TemplateId::SteamDeck => vec![Architecture::amd64],
            TemplateId::SteamOs => vec![Architecture::amd64],
            TemplateId::Tails => vec![Architecture::amd64],
            TemplateId::TrueNas => vec![Architecture::amd64],
            TemplateId::Ubuntu => vec![Architecture::amd64],
            TemplateId::Void => vec![Architecture::amd64],
            TemplateId::Windows10 => vec![Architecture::amd64],
            TemplateId::Windows11 => vec![Architecture::amd64],
            TemplateId::Windows7 => vec![Architecture::amd64],
            TemplateId::Zorin => vec![Architecture::amd64],
        }
    }

    /// Return whether the template supports multiboot.
    pub fn multiboot(&self) -> bool {
        match &self {
            TemplateId::Alpine => false,
            TemplateId::Arch => false,
            TemplateId::Artix => false,
            TemplateId::Bedrock => false,
            TemplateId::CentOs => false,
            TemplateId::Debian => false,
            TemplateId::ElementaryOs => false,
            TemplateId::Fedora => false,
            TemplateId::FreeBsd => false,
            TemplateId::Gentoo => false,
            TemplateId::Goldboot => false,
            TemplateId::Haiku => false,
            TemplateId::Kali => false,
            TemplateId::Mint => false,
            TemplateId::MacOs => false,
            TemplateId::Manjaro => false,
            TemplateId::NetBsd => false,
            TemplateId::NixOs => false,
            TemplateId::OpenBsd => false,
            TemplateId::OpenSuse => false,
            TemplateId::Oracle => false,
            TemplateId::Parrot => false,
            TemplateId::PopOs => false,
            TemplateId::Qubes => false,
            TemplateId::RedHat => false,
            TemplateId::Rocky => false,
            TemplateId::Slackware => false,
            TemplateId::SteamDeck => false,
            TemplateId::SteamOs => false,
            TemplateId::Tails => false,
            TemplateId::TrueNas => false,
            TemplateId::Ubuntu => false,
            TemplateId::Void => false,
            TemplateId::Windows10 => false,
            TemplateId::Windows11 => false,
            TemplateId::Windows7 => false,
            TemplateId::Zorin => false,
        }
    }

    pub fn new(&self) -> Box<dyn Template> {
        match &self {
            TemplateId::Alpine => Box::new(linux::alpine::AlpineTemplate::default()),
            TemplateId::Arch => Box::new(linux::arch::ArchTemplate::default()),
            TemplateId::Artix => todo!(),
            TemplateId::Bedrock => todo!(),
            TemplateId::CentOs => todo!(),
            TemplateId::Debian => Box::new(linux::debian::DebianTemplate::default()),
            TemplateId::ElementaryOs => todo!(),
            TemplateId::Fedora => todo!(),
            TemplateId::FreeBsd => todo!(),
            TemplateId::Gentoo => todo!(),
            TemplateId::Goldboot => Box::new(linux::goldboot::GoldbootTemplate::default()),
            TemplateId::Haiku => todo!(),
            TemplateId::Kali => todo!(),
            TemplateId::Mint => todo!(),
            TemplateId::MacOs => Box::new(macos::mac_os::MacOsTemplate::default()),
            TemplateId::Manjaro => todo!(),
            TemplateId::NetBsd => todo!(),
            TemplateId::NixOs => todo!(),
            TemplateId::OpenBsd => todo!(),
            TemplateId::OpenSuse => todo!(),
            TemplateId::Oracle => todo!(),
            TemplateId::Parrot => todo!(),
            TemplateId::PopOs => Box::new(linux::pop_os::PopOsTemplate::default()),
            TemplateId::Qubes => todo!(),
            TemplateId::RedHat => todo!(),
            TemplateId::Rocky => todo!(),
            TemplateId::Slackware => todo!(),
            TemplateId::SteamDeck => Box::new(linux::steam_deck::SteamDeckTemplate::default()),
            TemplateId::SteamOs => Box::new(linux::steam_os::SteamOsTemplate::default()),
            TemplateId::Tails => todo!(),
            TemplateId::TrueNas => todo!(),
            TemplateId::Ubuntu => Box::new(linux::ubuntu::UbuntuTemplate::default()),
            TemplateId::Void => todo!(),
            TemplateId::Windows10 => Box::new(windows::windows_10::Windows10Template::default()),
            TemplateId::Windows11 => todo!(),
            TemplateId::Windows7 => todo!(),
            TemplateId::Zorin => todo!(),
        }
    }
}
