//! Templates are the central concept that make it easy to define images.

use std::fmt::Display;

use super::sources::ImageSource;
use crate::foundry::FoundryWorker;
use anyhow::Result;
use arch_linux::ArchLinux;
use enum_dispatch::enum_dispatch;
use goldboot_image::ImageArch;
use serde::{Deserialize, Serialize};
use strum::EnumIter;

pub mod arch_linux;

/// "Casting" is the process of generating an immutable goldboot image from raw
/// configuration data.
///
/// This term comes from metallurgy where casting means to pour molten metal into
/// a mold, producing a solidified object in the shape of the mold.
#[enum_dispatch(ImageMold)]
pub trait CastImage {
    /// Cast an image from the mold.
    fn cast(&self, context: &FoundryWorker) -> Result<()>;
}

#[enum_dispatch(ImageMold)]
pub trait DefaultSource {
    fn default_source(&self) -> ImageSource;
}

/// Represents a "base configuration" that users can modify and use to build
/// images.
#[enum_dispatch]
#[derive(Clone, Serialize, Deserialize, Debug, EnumIter)]
pub enum ImageMold {
    // AlpineLinux,
    ArchLinux,
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

impl ImageMold {
    /// Supported system architectures
    pub fn architectures(&self) -> Vec<ImageArch> {
        todo!()
    }

    /// Whether the template can be combined with others in the same image
    pub fn alloy(&self) -> bool {
        false
    }

    // pub fn default_source
}

impl Display for ImageMold {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ImageMold::ArchLinux(_) => "Arch Linux",
            }
        )
    }
}

impl Default for ImageMold {
    fn default() -> Self {
        ImageMold::ArchLinux(ArchLinux::default())
    }
}
