use crate::config::Config;
use crate::config::Profile;
use crate::image::ImageMetadata;
use crate::packer::PackerProvisioner;
use crate::packer::PackerTemplate;
use crate::qemu::QemuConfig;
use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use log::debug;
use log::info;
use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use sysinfo::DiskExt;
use sysinfo::RefreshKind;
use sysinfo::System;
use sysinfo::SystemExt;
use tabled::Style;
use tabled::Table;

pub mod config;
pub mod image;
pub mod packer;
pub mod qemu;
pub mod windows;
pub mod profiles {
    pub mod arch_linux;
    pub mod pop_os_2104;
    pub mod pop_os_2110;
    pub mod windows_10;
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

    /// Run an existing image
    Run { image: String },

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
    Write { image: String, disk: String },

    /// Initialize the current directory
    Init { profile: Profile },
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

pub fn current_qemu_binary() -> &'static str {
    if cfg!(target_arch = "x86_64") {
        "qemu-system-x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "qemu-system-aarch64"
    } else {
        panic!("Unsupported platform");
    }
}

fn image_list() -> Result<()> {
    let images = ImageMetadata::load()?;

    print!("{}", Table::new(images).with(Style::modern()).to_string());
    Ok(())
}

fn build(cl: CommandLine) -> Result<()> {
    debug!("Starting build");

    // Load config
    let config = Config::load()?;

    // Acquire temporary directory for the build
    let tmp = tempfile::tempdir().unwrap();
    let context_path = tmp.path();
    debug!(
        "Allocated temporary directory for build: {}",
        context_path.display()
    );

    if let Some(profile) = &config.base {
        // Run profile-specific build hook
        let mut builder = match profile {
            Profile::ArchLinux => profiles::arch_linux::build(&config, &context_path),
            Profile::Windows10 => profiles::windows_10::build(&config, &context_path),
            Profile::PopOs2104 => profiles::pop_os_2104::build(&config, &context_path),
            Profile::PopOs2110 => profiles::pop_os_2110::build(&config, &context_path),
        }?;

        // Builder overrides
        builder.output_directory = context_path.to_str().unwrap().to_string();
        builder.vm_name = Some(config.name.clone());
        builder.qemuargs = Some(config.qemu.to_qemuargs());
        builder.memory = config.memory;
        builder.disk_size = config.disk_size;
        if let Some(arch) = &config.arch {
            builder.qemu_binary = match arch.as_str() {
                "x86_64" => Some("qemu-system-x86_64".into()),
                _ => None,
            };
        }

        if config.iso_url != "" {
            builder.iso_url = config.iso_url.clone();
        }

        if let Some(checksum) = config.iso_checksum {
            builder.iso_checksum = checksum;
        } else {
            builder.iso_checksum = "none".into();
        }

        // Create packer template
        let mut template = PackerTemplate::default();
        template.builders.push(builder);

        // Translate provisioners in config into packer provisioners
        for p in config.provisioners.iter() {
            let provisioner = match p.r#type.as_str() {
                "ansible" => PackerProvisioner {
                    r#type: "ansible".into(),
                    scripts: vec![],
                    playbook_file: Some(p.ansible.playbook.clone()),
                    user: Some("".into()),
                    use_proxy: Some(false),
                    extra_arguments: vec![
                        "-e".into(),
                        "ansible_winrm_scheme=http".into(),
                        "-e".into(),
                        "ansible_winrm_server_cert_validation=ignore".into(),
                    ],
                },
                _ => panic!(""),
            };
            template.provisioners.push(provisioner);
        }

        debug!("Created packer template: {:?}", &template);

        // Write the packer template
        fs::write(
            context_path.join("packer.json"),
            serde_json::to_string(&template).unwrap(),
        )
        .unwrap();
    }

    // Run the build
    if let Some(code) = Command::new("packer")
        .current_dir(context_path)
        .arg("build")
        .arg("-force")
        .arg("-on-error=abort")
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

    // Move the image to the library
    fs::rename(
        context_path.join(&config.name),
        image_library_path().join(format!("{}.qcow2", &metadata_name)),
    )
    .unwrap();

    return Ok(());
}

fn init(profile: &Profile) -> Result<()> {
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

    // Set current platform
    config.arch = if cfg!(target_arch = "x86_64") {
        Some("x86_64".into())
    } else if cfg!(target_arch = "aarch64") {
        Some("aarch64".into())
    } else {
        panic!("Unsupported platform");
    };

    // Choose the last disk as the target
    for disk in System::new_with_specifics(RefreshKind::new().with_disks_list()).disks() {
        config.disk_size = format!("{}b", disk.total_space());
    }

    // Run profile-specific initialization
    match profile {
        Profile::ArchLinux => profiles::arch_linux::init(&mut config),
        Profile::Windows10 => profiles::windows_10::init(&mut config),
        Profile::PopOs2104 => profiles::pop_os_2104::init(&mut config),
        Profile::PopOs2110 => profiles::pop_os_2110::init(&mut config),
    }

    // Finally write out the config
    fs::write(config_path, serde_json::to_string_pretty(&config).unwrap()).unwrap();

    Ok(())
}

fn write(image_name: &str, disk_name: &str) -> Result<()> {
    // TODO backup option

    // Locate the requested image
    let image = ImageMetadata::find(image_name)?;

    // Locate the requested disk
    if let Some(disk) = System::new_with_specifics(RefreshKind::new().with_disks_list())
        .disks()
        .iter()
        .find(|&d| d.name() == disk_name)
    {
        debug!("Found disk: {:?}", disk);

        // Verify sizes are compatible
        if image.size != disk.total_space() {
            bail!("The requested disk size is not equal to the image size");
        }

    	// Check if mounted
    	// TODO
    } else {
        bail!("Disk not found: {}", disk_name);
    }
    Ok(())
}

fn run(image: &str) -> Result<()> {
    Command::new("qemu-system-x86_64").args([
        "-display",
        "gtk",
        "-machine",
        "type=pc,accel=kvm",
        "-m",
        "1000M",
        "-boot",
        "once=d",
        "-drive",
        "file=/var/lib/goldboot/images/da1d9c276e89c1a7cdc27fe6b52b1449e6d0feb9c7f9ac38873210f4359f0642,if=virtio,cache=writeback,discard=ignore,format=qcow2",
        "-name",
        "cli",
    ])
    .status().unwrap();
    Ok(())
}

/// Determine whether builds should be headless or not for debugging.
pub fn build_headless_debug() -> bool {
    match env::var("GOLDBOOT_DEBUG") {
        Ok(_) => false,
        Err(_) => true,
    }
}

pub fn main() -> Result<()> {
    let cl = CommandLine::parse();

    // Configure logging
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    match &cl.command {
        Commands::Build {} => build(cl),
        Commands::Run { image } => run(image),
        Commands::Registry { command } => build(cl),
        Commands::Write { image, disk } => write(image, disk),
        Commands::Init { profile } => init(profile),
        Commands::Image { command } => match &command {
            ImageCommands::List {} => image_list(),
            ImageCommands::Info {} => image_list(),
        },
    }
}
