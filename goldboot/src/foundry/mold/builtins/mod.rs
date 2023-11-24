//! Templates are the central concept that make it easy to define images.

use serde::{Deserialize, Serialize};
use std::{error::Error, fmt::Display, path::Path};
use strum::EnumIter;

use crate::FoundryWorker;

pub mod arch_linux;

pub struct TemplateMetadata {
    /// The image full name
    pub name: String,

    /// Supported system architectures
    pub architectures: Vec<Architecture>,

    /// Whether the template can be combined with others in the same image
    pub multiboot: bool,
}

/// "Casting" is the process of generating an immutable goldboot image from raw
/// configuration data.
///
/// This term comes from metallurgy where casting means to pour molten metal into
/// a mold, producing a solidified object in the shape of the mold.
pub trait ImageMold: Default + Serialize + PromptMut {
    /// Cast an image from the mold.
    fn cast(&self, context: &FoundryWorker) -> Result<(), Box<dyn Error>>;

    // ///
    // fn metadata() -> TemplateMetadata;
}

/// Represents a "base configuration" that users can modify and use to build
/// images.
#[derive(Clone, Serialize, Deserialize, Debug, EnumIter)]
#[serde(tag = "base")]
pub enum Mold {
    // AlpineLinux(crate::molds::alpine_linux::AlpineLinux),
    ArchLinux(crate::molds::arch_linux::ArchLinux),
    // Artix,
    // BedrockLinux,
    // CentOs,
    // Debian,
    // ElementaryOs,
    // Fedora,
    // FreeBsd,
    // Gentoo,
    // GoldbootLinux,
    // Haiku,
    // Kali,
    // LinuxMint,
    // MacOs,
    // Manjaro,
    // NetBsd,
    // NixOs,
    // OpenBsd,
    // OpenSuse,
    // Oracle,
    // Parrot,
    // PopOs,
    // Qubes,
    // RedHat,
    // RockyLinux,
    // Slackware,
    // SteamDeck,
    // SteamOs,
    // Tails,
    // TrueNas,
    // Ubuntu,
    // VoidLinux,
    // Windows10,
    // Windows11,
    // Windows7,
    // Zorin,
}

impl Display for Mold {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", todo!())
    }
}
