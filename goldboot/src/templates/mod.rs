//! Templates are the central concept that make it easy to define images.

use crate::{build::BuildWorker, *};
use serde::{Deserialize, Serialize};
use std::{error::Error, fmt::Display, path::Path};

pub mod arch_linux;

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
    Arch(templates::linux::arch::ArchTemplate),
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
				Template::Alpine(_)       => "Alpine Linux",
				Template::Arch(_)         => "Arch Linux",
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
            Template::Alpine(_) => vec![Architecture::amd64, Architecture::i386],
            Template::Arch(_) => vec![Architecture::amd64, Architecture::i386],
            Template::Artix => vec![Architecture::amd64],
            Template::Bedrock => vec![Architecture::amd64],
            Template::CentOs => vec![Architecture::amd64],
            Template::Debian => vec![Architecture::amd64],
            Template::ElementaryOs => vec![Architecture::amd64],
            Template::Fedora => vec![Architecture::amd64],
            Template::FreeBsd => vec![Architecture::amd64],
            Template::Gentoo => vec![Architecture::amd64],
            Template::Goldboot => vec![Architecture::amd64],
            Template::Haiku => vec![Architecture::amd64],
            Template::Kali => vec![Architecture::amd64],
            Template::Mint => vec![Architecture::amd64],
            Template::MacOs => vec![Architecture::amd64],
            Template::Manjaro => vec![Architecture::amd64],
            Template::NetBsd => vec![Architecture::amd64],
            Template::NixOs => vec![Architecture::amd64],
            Template::OpenBsd => vec![Architecture::amd64],
            Template::OpenSuse => vec![Architecture::amd64],
            Template::Oracle => vec![Architecture::amd64],
            Template::Parrot => vec![Architecture::amd64],
            Template::PopOs => vec![Architecture::amd64],
            Template::Qubes => vec![Architecture::amd64],
            Template::RedHat => vec![Architecture::amd64],
            Template::Rocky => vec![Architecture::amd64],
            Template::Slackware => vec![Architecture::amd64],
            Template::SteamDeck => vec![Architecture::amd64],
            Template::SteamOs => vec![Architecture::amd64],
            Template::Tails => vec![Architecture::amd64],
            Template::TrueNas => vec![Architecture::amd64],
            Template::Ubuntu => vec![Architecture::amd64],
            Template::Void => vec![Architecture::amd64],
            Template::Windows10 => vec![Architecture::amd64],
            Template::Windows11 => vec![Architecture::amd64],
            Template::Windows7 => vec![Architecture::amd64],
            Template::Zorin => vec![Architecture::amd64],
        }
    }

    /// Return whether the template supports multiboot.
    pub fn multiboot(&self) -> bool {
        match &self {
            Template::Alpine => false,
            Template::Arch => false,
            Template::Artix => false,
            Template::Bedrock => false,
            Template::CentOs => false,
            Template::Debian => false,
            Template::ElementaryOs => false,
            Template::Fedora => false,
            Template::FreeBsd => false,
            Template::Gentoo => false,
            Template::Goldboot => false,
            Template::Haiku => false,
            Template::Kali => false,
            Template::Mint => false,
            Template::MacOs => false,
            Template::Manjaro => false,
            Template::NetBsd => false,
            Template::NixOs => false,
            Template::OpenBsd => false,
            Template::OpenSuse => false,
            Template::Oracle => false,
            Template::Parrot => false,
            Template::PopOs => false,
            Template::Qubes => false,
            Template::RedHat => false,
            Template::Rocky => false,
            Template::Slackware => false,
            Template::SteamDeck => false,
            Template::SteamOs => false,
            Template::Tails => false,
            Template::TrueNas => false,
            Template::Ubuntu => false,
            Template::Void => false,
            Template::Windows10 => false,
            Template::Windows11 => false,
            Template::Windows7 => false,
            Template::Zorin => false,
        }
    }

    pub fn new(&self) -> Box<dyn Template> {
        match &self {
            Template::Alpine => Box::new(linux::alpine::AlpineTemplate::default()),
            Template::Arch => Box::new(linux::arch::ArchTemplate::default()),
            Template::Artix => todo!(),
            Template::Bedrock => todo!(),
            Template::CentOs => todo!(),
            Template::Debian => Box::new(linux::debian::DebianTemplate::default()),
            Template::ElementaryOs => todo!(),
            Template::Fedora => todo!(),
            Template::FreeBsd => todo!(),
            Template::Gentoo => todo!(),
            Template::Goldboot => Box::new(linux::goldboot::GoldbootTemplate::default()),
            Template::Haiku => todo!(),
            Template::Kali => todo!(),
            Template::Mint => todo!(),
            Template::MacOs => Box::new(macos::mac_os::MacOsTemplate::default()),
            Template::Manjaro => todo!(),
            Template::NetBsd => todo!(),
            Template::NixOs => todo!(),
            Template::OpenBsd => todo!(),
            Template::OpenSuse => todo!(),
            Template::Oracle => todo!(),
            Template::Parrot => todo!(),
            Template::PopOs => Box::new(linux::pop_os::PopOsTemplate::default()),
            Template::Qubes => todo!(),
            Template::RedHat => todo!(),
            Template::Rocky => todo!(),
            Template::Slackware => todo!(),
            Template::SteamDeck => Box::new(linux::steam_deck::SteamDeckTemplate::default()),
            Template::SteamOs => Box::new(linux::steam_os::SteamOsTemplate::default()),
            Template::Tails => todo!(),
            Template::TrueNas => todo!(),
            Template::Ubuntu => Box::new(linux::ubuntu::UbuntuTemplate::default()),
            Template::Void => todo!(),
            Template::Windows10 => Box::new(windows::windows_10::Windows10Template::default()),
            Template::Windows11 => todo!(),
            Template::Windows7 => todo!(),
            Template::Zorin => todo!(),
        }
    }
}
