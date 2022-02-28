use crate::packer::PackerTemplate;
use crate::packer::PackerProvisioner;
use std::path::PathBuf;
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
pub mod windows;
pub mod profiles {
	pub mod arch_linux;
}

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct CommandLine {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Build new image
    Build {},

    /// Manage image registries
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

/// Return the image library path for the current platform.
pub fn image_library_path() -> PathBuf {
	if cfg!(target_os = "linux") {
		PathBuf::from("/var/lib/goldboot/images")
	} else {
		panic!("Unsupported platform");
	}
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

    // Generate packer builder according to profile
    let mut builder = match config.base.as_str() {
    	"ArchLinux" => profiles::arch_linux::default_builder(),
    	_ => bail!("Unknown profile"),
    };

    // Builder overrides
    builder.output_directory = Some(image_library_path().to_str().unwrap().to_string());
    builder.vm_name = Some(config.name.clone());
    builder.qemuargs = Some(config.qemu.to_qemuargs());

    if let Some(iso_url) = config.iso_url {
    	builder.iso_url = Some(iso_url);
    }

    if let Some(iso_checksum) = config.iso_checksum {
    	builder.iso_checksum = Some(iso_checksum);
    }

    // Create packer template
    let mut template = PackerTemplate::default();
    template.builders.push(builder);

    // Translate provisioners in config into packer provisioners
    for p in config.provisioners.iter() {
        let provisioner = match p.r#type.as_str() {
            "ansible" => {
                PackerProvisioner {
                    r#type: "ansible".into(),
                    scripts: vec![],
                    playbook_file: Some(p.ansible.playbook.clone()),
                    user: Some("".into()),
                    use_proxy: Some(false),
                    extra_arguments: vec![
                        "-e".into(), "ansible_winrm_scheme=http".into(),
                        "-e".into(), "ansible_winrm_server_cert_validation=ignore".into()
                    ],
                }
            },
            _ => panic!(""),
        };
        template.provisioners.push(provisioner);
    }

    // Write the packer template
    fs::write(
        tmp.join("packer.json"),
        serde_json::to_string(&template).unwrap(),
    )
    .unwrap();

    // Run the build
    if let Some(code) = Command::new("packer")
        .current_dir(tmp)
        .arg("build")
        .arg("-force")
        .arg("packer.json")
        .status()
        .expect("Failed to launch packer")
        .code()
    {
        if code != 0 {
        	bail!("Build failed with error code: {}", code);
        }
    } else {
    	bail!("");
    }

    debug!("Build completed successfully");

    // Create new image metadata
    let metadata_name = ImageMetadata::new(image_library_path().join(&config.name))?.write()?;

    // Rename the image itself
    fs::rename(image_library_path().join(&config.name), image_library_path().join(&metadata_name)).unwrap();

    return Ok(());
}

fn init(profile: &str) -> Result<()> {
    let config_path = Path::new("goldboot.json");

    if config_path.exists() {
        bail!("This directory has already been initialized. Delete goldboot.json to reinitialize.");
    }

    let mut config = Config::default();

    // Set name equal to directory name
    if let Some(name) = env::current_dir().unwrap().file_name() {
        config.name = name.to_str().unwrap().to_string();
    }

    // Generate QEMU flags for this hardware
    config.qemu = QemuConfig::generate_config()?;

    // Set base profile
    config.base = profile.to_string();

    // Allow profile-specific initialization
    match profile {
    	"ArchLinux" => profiles::arch_linux::init(&mut config),
    	_ => bail!("Unknown profile"),
    }

    // Finally write out the config
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
        Commands::Init { profile } => init(profile),
        Commands::Image { command } => match &command {
            ImageCommands::List {} => image_list(),
            ImageCommands::Info {} => image_list(),
        },
    }
}
