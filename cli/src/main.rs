use crate::qemu::QemuConfig;
use crate::config::Config;
use crate::image::ImageMetadata;
use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use log::debug;
use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

pub mod config;
pub mod image;
pub mod packer;
pub mod qemu;

/// Goldboot CLI
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct CommandLine {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Build image
    Build {},

    Registry {
        #[clap(subcommand)]
        command: RegistryCommands,
    },

    /// Manage local images
    Image {
        #[clap(subcommand)]
        command: ImageCommands,
    },

    /// Write image to a disk
    Write {},

    /// Initialize the current directory
    Init { profile: String },
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

    Info {},
}

fn image_list() -> Result<()> {
    for metadata in ImageMetadata::load()? {
        println!("{}", metadata.name);
    }
    Ok(())
}

fn build(cl: CommandLine) -> Result<()> {
    debug!("Starting build");

    // Load config
    let config = Config::load()?;

    // Acquire temporary directory for the build
    //let tmp = tempfile::tempdir().unwrap();
    let tmp = Path::new("/tmp/testpacker");
    debug!("Allocated temporary directory for build: {}", tmp.display());

    // Generate packer template
    let template = config.generate_packer_template()?;

    fs::write(
        tmp.join("packer.json"),
        serde_json::to_string(&template).unwrap(),
    )
    .unwrap();

    if let Some(code) = Command::new("packer")
        .current_dir(tmp)
        .arg("build")
        .arg("packer.json")
        .status()
        .expect("Failed to launch packer")
        .code()
    {
        if code == 0 {
            debug!("Build completed successfully");
            
            // Create new image metadata
            //ImageMetadata::write();
        }
    } else {
    	bail!("");
    }
}

fn init(cl: CommandLine) -> Result<()> {
    let config_path = Path::new("goldboot.json");

    if config_path.exists() {
        bail!("This directory has already been initialized");
    }

    let mut config = Config::default();

    // Set name equal to directory name
    if let Some(name) = env::current_dir().unwrap().file_name() {
        config.name = name.to_str().unwrap().to_string();
    }

    // Generate QEMU flags for this hardware
    config.qemu = QemuConfig::generate_config()?;

    // Write out the config
    fs::write(config_path, serde_json::to_string_pretty(&config).unwrap()).unwrap();

    Ok(())
}

pub fn main() -> Result<()> {
    let cl = CommandLine::parse();

    // Configure logging
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    match &cl.command {
        Commands::Build {} => build(cl),
        Commands::Registry { command } => build(cl),
        Commands::Write {} => build(cl),
        Commands::Init { profile } => init(cl),
        Commands::Image { command } => match &command {
            ImageCommands::List {} => image_list(),
            ImageCommands::Info {} => image_list(),
        },
    }
}
