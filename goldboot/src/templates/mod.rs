//! Templates are the central concept that make it easy to define images.

use crate::templates::arch_linux::ArchLinuxTemplate;
use crate::{build::BuildWorker, *};
use serde::{Deserialize, Serialize};
use std::{error::Error, fmt::Display, path::Path};

pub mod arch_linux;

pub struct TemplateMetadata {
    /// The image full name
    pub name: String,

    /// Supported system architectures
    pub architectures: Vec<Architecture>,

    /// Whether the template can be combined with others in the same image
    pub multiboot: bool,
}

pub trait BuildTemplate {
    /// Build an image from the template.
    fn build(&self, context: &BuildWorker) -> Result<(), Box<dyn Error>>;

    // ///
    // fn metadata() -> TemplateMetadata;
}

/// Represents a "base configuration" that users can modify and use to build
/// images.
#[derive(Clone, Serialize, Deserialize, Debug, EnumIter)]
#[serde(tag = "base")]
pub enum Template {
    // AlpineLinux(AlpineLinuxTemplate),
    ArchLinux(ArchLinuxTemplate),
    Artix,
    BedrockLinux,
    CentOs,
    Debian,
    ElementaryOs,
    Fedora,
    FreeBsd,
    Gentoo,
    GoldbootLinux,
    Haiku,
    Kali,
    LinuxMint,
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
    RockyLinux,
    Slackware,
    SteamDeck,
    SteamOs,
    Tails,
    TrueNas,
    Ubuntu,
    VoidLinux,
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
    	    "{}",todo!()
		)
	}
}
