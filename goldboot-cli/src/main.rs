use clap::{Parser, Subcommand};
use colored::*;
use sha2::{Digest, Sha256};
use std::{env, error::Error, path::PathBuf};

pub mod image;
pub mod init;
pub mod make_usb;
pub mod registry;

#[rustfmt::skip]
fn print_banner() {
	println!("⬜{}⬜", "⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜");
	println!("⬜{}⬜", "　　　　　　　　⬛　　　⬛　⬛　　　　　　　　　　　⬛　".truecolor(200, 171, 55));
	println!("⬜{}⬜", "　　　　　　　　⬛　　　⬛　⬛　　　　　　　　　　　⬛⬛".truecolor(200, 171, 55));
	println!("⬜{}⬜", "⬛⬛⬛　⬛⬛⬛　⬛　⬛⬛⬛　⬛⬛⬛　⬛⬛⬛　⬛⬛⬛　⬛　".truecolor(200, 171, 55));
	println!("⬜{}⬜", "⬛　⬛　⬛　⬛　⬛　⬛　⬛　⬛　⬛　⬛　⬛　⬛　⬛　⬛　".truecolor(200, 171, 55));
	println!("⬜{}⬜", "⬛⬛⬛　⬛⬛⬛　⬛　⬛⬛⬛　⬛⬛⬛　⬛⬛⬛　⬛⬛⬛　⬛⬛".truecolor(200, 171, 55));
	println!("⬜{}⬜", "　　⬛　　　　　　　　　　　　　　　　　　　　　　　　　".truecolor(200, 171, 55));
	println!("⬜{}⬜", "⬛⬛⬛　　　　　　　　　　　　　　　　　　　　　　　　　".truecolor(200, 171, 55));
	println!("⬜{}⬜", "⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜");
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
		/// Save a screenshot to ./debug after each boot command
		#[clap(long, takes_value = false)]
		record: bool,

		/// Insert a breakpoint after each boot command
		#[clap(long, takes_value = false)]
		debug: bool,
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

		/// A base template (which can be found with --list-templates)
		#[clap(long)]
		template: Vec<String>,

		/// The amount of memory the image can access
		#[clap(long)]
		memory: Option<String>,

		/// The amount of storage the image can access
		#[clap(long)]
		disk: Option<String>,

		#[clap(long, takes_value = false)]
		mimic_hardware: bool,

		/// List available templates and exit
		#[clap(long, takes_value = false)]
		list_templates: bool,
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
		#[clap(long)]
		image: String,

		/// The disk to overwrite
		#[clap(long)]
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

pub fn main() -> Result<(), Box<dyn Error>> {
	// Parse command line first
	let cl = CommandLine::parse();

	// Configure logging
	env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

	// Dispatch command
	match &cl.command {
		Commands::Build { record, debug } => {
			print_banner();
			debug!("Loading config from ./goldboot.json");

			// Load build config from current directory
			let config: BuildConfig = serde_json::from_slice(&std::fs::read("goldboot.json")?)?;
			debug!("Loaded config: {:#?}", &config);

			// Fully verify config before proceeding
			config.validate()?;

			// Run the build finally
			let mut job = BuildJob::new(config, *record, *debug, !env::var("CI").is_ok());
			job.run()?;
		}
		Commands::Registry { command } => match &command {
			RegistryCommands::Push { url } => crate::registry::push(),
			RegistryCommands::Pull { url } => crate::registry::pull(),
		},
		Commands::Init {
			name,
			template,
			memory,
			disk,
			list_templates,
		} => {
			if *list_templates {
				profile::list_profiles()
			} else {
				crate::init::init(template, name, memory, disk)
			}
		}
		Commands::MakeUsb {
			disk,
			confirm,
			include,
		} => crate::make_usb::make_usb(),
		Commands::Image { command } => match &command {
			ImageCommands::List {} => crate::image::list(),
			ImageCommands::Info { image } => crate::image::info(image),
			ImageCommands::Run { image } => crate::image::run(image),
			ImageCommands::Write {
				image,
				disk,
				confirm,
			} => crate::image::write(image, disk),
		},
	}
}
