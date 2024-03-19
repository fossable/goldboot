use super::sources::ImageSource;
use crate::cli::prompt::Prompt;
use crate::foundry::Foundry;
use crate::foundry::FoundryWorker;
use anyhow::Result;
use clap::ValueEnum;
use dialoguer::theme::Theme;
use enum_dispatch::enum_dispatch;
use goldboot_image::ImageArch;
use serde::{Deserialize, Serialize};
use std::{fmt::Display, sync::OnceLock};
use strum::{EnumIter, IntoEnumIterator};

use alpine_linux::AlpineLinux;
use arch_linux::ArchLinux;
use debian::Debian;

pub mod alpine_linux;
pub mod arch_linux;
pub mod debian;

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
    fn default_source(&self, arch: ImageArch) -> Result<ImageSource>;
}

/// Represents a "base configuration" that users can modify and use to build
/// images.
#[enum_dispatch]
#[derive(Clone, Serialize, Deserialize, Debug, EnumIter)]
pub enum ImageMold {
    AlpineLinux,
    ArchLinux,
    // Artix,
    // BedrockLinux,
    // CentOs,
    Debian,
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
        match self {
            ImageMold::AlpineLinux(_) => vec![ImageArch::Amd64, ImageArch::Arm64],
            ImageMold::ArchLinux(_) => vec![ImageArch::Amd64],
            ImageMold::Debian(_) => vec![ImageArch::Amd64, ImageArch::Arm64],
        }
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
                ImageMold::AlpineLinux(_) => "AlpineLinux",
                ImageMold::ArchLinux(_) => "ArchLinux",
                ImageMold::Debian(_) => "Debian",
            }
        )
    }
}

impl Default for ImageMold {
    fn default() -> Self {
        ImageMold::ArchLinux(ArchLinux::default())
    }
}

static VARIANTS: OnceLock<Vec<ImageMold>> = OnceLock::new();

impl ValueEnum for ImageMold {
    fn value_variants<'a>() -> &'a [Self] {
        VARIANTS.get_or_init(|| ImageMold::iter().collect())
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        Some(clap::builder::PossibleValue::new(
            Into::<clap::builder::Str>::into(self.to_string()),
        ))
    }
}
