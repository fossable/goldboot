use crate::builder::Builder;
use crate::cli::prompt::Prompt;
use anyhow::Result;
use clap::ValueEnum;
use enum_dispatch::enum_dispatch;
use goldboot_image::ImageArch;
#[cfg(feature = "config-python")]
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use std::{fmt::Display, sync::OnceLock};
use strum::{EnumIter, IntoEnumIterator};

use alpine_linux::AlpineLinux;
use arch_linux::ArchLinux;
use debian::Debian;
// use goldboot::Goldboot;
use nix::Nix;
use windows_10::Windows10;
use windows_11::Windows11;

pub mod alpine_linux;
pub mod arch_linux;
pub mod debian;
// pub mod goldboot;
pub mod nix;
pub mod windows_10;
pub mod windows_11;

/// "Building" is the process of generating an immutable goldboot image from raw
/// configuration data.
#[enum_dispatch(Os)]
pub trait BuildImage {
    /// Build an image.
    fn build(&self, builder: &Builder) -> Result<()>;
}

// TODO Element?

/// Represents a "base configuration" that users can modify and use to build
/// images.
#[enum_dispatch]
#[derive(Clone, Serialize, Deserialize, Debug, EnumIter)]
#[serde(tag = "os")]
pub enum Os {
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
    // Goldboot,
    // Haiku,
    // Kali,
    // LinuxMint,
    // MacOs,
    // Manjaro,
    // NetBsd,
    Nix,
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
    Windows10,
    Windows11,
    // Windows7,
    // Zorin,
}

impl Os {
    /// Supported system architectures
    pub fn architectures(&self) -> Vec<ImageArch> {
        match self {
            Os::AlpineLinux(_) => vec![ImageArch::Amd64, ImageArch::Arm64],
            Os::ArchLinux(_) => vec![ImageArch::Amd64],
            Os::Debian(_) => vec![ImageArch::Amd64, ImageArch::Arm64],
            // Os::Goldboot(_) => vec![ImageArch::Amd64, ImageArch::Arm64],
            Os::Nix(_) => vec![ImageArch::Amd64, ImageArch::Arm64],
            Os::Windows10(_) => vec![ImageArch::Amd64],
            Os::Windows11(_) => vec![ImageArch::Amd64],
        }
    }

    /// Whether the template can be combined with others in the same image
    pub fn alloy(&self) -> bool {
        false
    }

    // pub fn default_source
}

impl Display for Os {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Os::AlpineLinux(_) => "AlpineLinux",
                Os::ArchLinux(_) => "ArchLinux",
                Os::Debian(_) => "Debian",
                // Os::Goldboot(_) => "Goldboot",
                Os::Nix(_) => "NixOS",
                Os::Windows10(_) => "Windows10",
                Os::Windows11(_) => "Windows11",
            }
        )
    }
}

impl Default for Os {
    fn default() -> Self {
        Os::ArchLinux(ArchLinux::default())
    }
}

#[cfg(feature = "config-python")]
impl<'py> FromPyObject<'py> for Os {
    fn extract_bound(ob: &Bound<'py, PyAny>) -> PyResult<Self> {
        pythonize::depythonize(ob)
            .map_err(|e| pyo3::exceptions::PyTypeError::new_err(e.to_string()))
    }
}

static VARIANTS: OnceLock<Vec<Os>> = OnceLock::new();

impl ValueEnum for Os {
    fn value_variants<'a>() -> &'a [Self] {
        VARIANTS.get_or_init(|| Os::iter().collect())
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        Some(clap::builder::PossibleValue::new(
            Into::<clap::builder::Str>::into(self.to_string()),
        ))
    }
}

// impl From<Element> for ElementHeader {
//     fn from(value: Element) -> ElementHeader {
//         todo!()
//     }
// }

#[macro_export]
macro_rules! size {
    ($element:expr) => {
        $element.size
    };
}
