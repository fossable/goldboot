use clap::{Parser, Subcommand};
use std::{env, error::Error, path::PathBuf};

pub mod config;
pub mod packer;
pub mod profile;
pub mod qemu;
pub mod windows;
pub mod profiles {
    pub mod arch_linux;
    pub mod pop_os_2110;
    pub mod ubuntu_server_2110;
    pub mod windows_10;
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
    Build {},

    /// Manage local images
    Image {
        #[clap(subcommand)]
        command: ImageCommands,
    },

    /// Initialize the current directory
    Init {
        profile: Option<String>,
        template: Option<String>,
    },

    /// Create a bootable USB drive
    MakeUsb {
        /// The disk to erase and make bootable
        disk: String,

        /// Do not check for confirmation
        #[clap(long, takes_value = false)]
        confirm: bool,
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
    match env::var("GOLDBOOT_DEBUG") {
        Ok(_) => false,
        Err(_) => true,
    }
}

pub fn main() -> Result<(), Box<dyn Error>> {
    // Parse command line first
    let cl = CommandLine::parse();

    // Configure logging
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    // Dispatch command
    match &cl.command {
        Commands::Build {} => commands::build::build(),
        Commands::Registry { command } => match &command {
            RegistryCommands::Push { url } => commands::registry::push(),
            RegistryCommands::Pull { url } => commands::registry::pull(),
        },
        Commands::Init { profile, template } => commands::init::init(profile.to_owned(), template.to_owned()),
        Commands::MakeUsb { disk, confirm } => commands::make_usb::make_usb(),
        Commands::Image { command } => match &command {
            ImageCommands::List {} => commands::image::list(),
            ImageCommands::Info { image } => commands::image::info(image),
            ImageCommands::Run { image } => commands::image::run(image),
            ImageCommands::Write { image, disk, confirm } => commands::image::write(image, disk),
        },
    }
}
