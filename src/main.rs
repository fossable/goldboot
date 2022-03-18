#![feature(derive_default_enum)]

use clap::{Parser, Subcommand};
use sha2::{Digest, Sha256};
use std::{
    env,
    error::Error,
    path::{Path, PathBuf},
};

pub mod config;
pub mod packer;
pub mod profile;
pub mod qemu;
pub mod windows;
pub mod profiles {
    pub mod arch_linux;
    pub mod debian;
    pub mod pop_os;
    pub mod steam_deck;
    pub mod steam_os;
    pub mod ubuntu_desktop;
    pub mod ubuntu_server;
    pub mod windows_10;
    pub mod windows_11;
    pub mod windows_7;
}
pub mod commands {
    pub mod build;
    pub mod image;
    pub mod init;
    pub mod make_usb;
    pub mod registry;
}

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct CommandLine {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Build a new image
    Build {
        /// Scale all wait times to account for hardware of different speeds
        #[clap(long)]
        scale: Option<f64>
    },

    /// Manage local images
    Image {
        #[clap(subcommand)]
        command: ImageCommands,
    },

    /// Initialize the current directory
    Init {
        /// The image name
        #[clap(long)]
        name: Option<String>,

        /// A base profile which can be found with --list-profiles
        #[clap(long)]
        profile: Vec<String>,

        /// The amount of memory the image can access
        #[clap(long)]
        memory: Option<String>,

        /// The amount of storage the image can access
        #[clap(long)]
        disk: Option<String>,

        /// List available profiles and exit
        #[clap(long, takes_value = false)]
        list_profiles: bool,

        /// An existing packer template
        #[clap(long)]
        template: Option<String>,
    },

    /// Create a bootable USB drive
    MakeUsb {
        /// The disk to erase and make bootable
        disk: String,

        /// Do not check for confirmation
        #[clap(long, takes_value = false)]
        confirm: bool,

        /// A local image to include on the boot USB
        #[clap(long)]
        include: Vec<String>,
    },

    /// Manage image registries
    Registry {
        #[clap(subcommand)]
        command: RegistryCommands,
    },
}

#[derive(Subcommand, Debug)]
enum RegistryCommands {
    /// Upload a local image to a remote registry
    Push { url: String },

    /// Download an image from a remote registry
    Pull { url: String },
}

#[derive(Subcommand, Debug)]
enum ImageCommands {
    /// List local images
    List {},

    Info {
        image: String,
    },

    /// Write image to a disk
    Write {
        /// The selected image
        image: String,

        /// The disk to overwrite
        disk: String,

        /// Do not check for confirmation
        #[clap(long, takes_value = false)]
        confirm: bool,
    },

    /// Run an existing image
    Run {
        image: String,
    },
}

/// Return the image library path for the current platform.
pub fn image_library_path() -> PathBuf {
    if cfg!(target_os = "linux") {
        PathBuf::from("/var/lib/goldboot/images")
    } else {
        panic!("Unsupported platform");
    }
}

/// Search filesystem for UEFI firmware
pub fn ovmf_firmware() -> Option<String> {
    for path in vec![
        "/usr/share/ovmf/x64/OVMF.fd",
        "/usr/share/OVMF/OVMF_CODE.fd",
    ] {
        if Path::new(&path).is_file() {
            return Some(path.to_string());
        }
    }
    None
}

/// A simple cache for storing images that are not stored in the Packer cache.
/// Most images here need some kind of transformation before they are bootable.
pub fn image_cache_lookup(key: &str) -> PathBuf {
    // Hash the key to get the filename
    let hash = hex::encode(Sha256::new().chain_update(&key).finalize());

    if cfg!(target_os = "linux") {
        PathBuf::from("/var/lib/goldboot/cache").join(hash)
    } else {
        panic!("Unsupported platform");
    }
}

/// Get the QEMU system binary for the current platform
pub fn current_qemu_binary() -> &'static str {
    if cfg!(target_arch = "x86_64") {
        "qemu-system-x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "qemu-system-aarch64"
    } else {
        panic!("Unsupported platform");
    }
}

/// Determine whether builds should be headless or not for debugging.
pub fn build_headless_debug() -> bool {
    if env::var("CI").is_ok() {
        return true;
    }
    if env::var("GOLDBOOT_DEBUG").is_ok() {
        return false;
    }
    return true;
}

/// Scale all wait times to support faster or slower machines.
pub fn scale_wait_time(seconds: u32) -> String {
    unsafe { format!("{}s", (seconds as f64 * MULTIPLIER) as u64) }
}

static mut MULTIPLIER: f64 = 1f64;

pub fn main() -> Result<(), Box<dyn Error>> {
    // Parse command line first
    let cl = CommandLine::parse();

    // Configure logging
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    // Dispatch command
    match &cl.command {
        Commands::Build { scale } => {
            // Set global multiplier
            // TODO: unsafe can be refactored
            if let Some(m) = scale {
                unsafe {
                    MULTIPLIER = *m;
                }
            }
            commands::build::build()
        }
        Commands::Registry { command } => match &command {
            RegistryCommands::Push { url } => commands::registry::push(),
            RegistryCommands::Pull { url } => commands::registry::pull(),
        },
        Commands::Init {
            name,
            profile,
            memory,
            disk,
            list_profiles,
            template,
        } => {
            if *list_profiles {
                profile::list_profiles()
            } else {
                commands::init::init(profile, template, name, memory, disk)
            }
        }
        Commands::MakeUsb {
            disk,
            confirm,
            include,
        } => commands::make_usb::make_usb(),
        Commands::Image { command } => match &command {
            ImageCommands::List {} => commands::image::list(),
            ImageCommands::Info { image } => commands::image::info(image),
            ImageCommands::Run { image } => commands::image::run(image),
            ImageCommands::Write {
                image,
                disk,
                confirm,
            } => commands::image::write(image, disk),
        },
    }
}
