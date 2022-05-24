use chrono::TimeZone;
use clap::{Parser, Subcommand};
use colored::*;
use goldboot_core::{build::BuildJob, image::library::ImageLibrary, BuildConfig};
use log::debug;
use std::{env, error::Error, fs::File, path::Path};
use ubyte::ToByteUnit;
use validator::Validate;

pub mod init;
pub mod registry;

#[rustfmt::skip]
fn print_banner() {
	if cfg!(target_os = "linux") {
		println!("{}", "");
		println!("  {}", "　　　　　　　　⬛　　　⬛　⬛　　　　　　　　　　　⬛　".truecolor(200, 171, 55));
		println!("  {}", "　　　　　　　　⬛　　　⬛　⬛　　　　　　　　　　　⬛⬛".truecolor(200, 171, 55));
		println!("  {}", "⬛⬛⬛　⬛⬛⬛　⬛　⬛⬛⬛　⬛⬛⬛　⬛⬛⬛　⬛⬛⬛　⬛　".truecolor(200, 171, 55));
		println!("  {}", "⬛　⬛　⬛　⬛　⬛　⬛　⬛　⬛　⬛　⬛　⬛　⬛　⬛　⬛　".truecolor(200, 171, 55));
		println!("  {}", "⬛⬛⬛　⬛⬛⬛　⬛　⬛⬛⬛　⬛⬛⬛　⬛⬛⬛　⬛⬛⬛　⬛⬛".truecolor(200, 171, 55));
		println!("  {}", "　　⬛　　　　　　　　　　　　　　　　　　　　　　　　　".truecolor(200, 171, 55));
		println!("  {}", "⬛⬛⬛　　　　　　　　　　　　　　　　　　　　　　　　　".truecolor(200, 171, 55));
		println!("{}", "");
	} else if cfg!(target_os = "macos") {
		// TODO fix color
	}
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

	/// Write images to storage
	Write {
		/// The ID of the image to write
		#[clap(long)]
		image: String,

		/// The output destination
		#[clap(long)]
		output: String,

		/// Do not prompt for confirmation (be extremely careful with this)
		#[clap(long, takes_value = false)]
		confirm: bool,
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
		output: String,

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

	/// Run an existing image
	Run {
		image: String,
	},
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
			debug!("Loaded: {:#?}", &config);

			// Fully verify config before proceeding
			config.validate()?;

			// Run the build finally
			let mut job = BuildJob::new(config, *record, *debug);
			job.run()
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
			mimic_hardware,
		} => {
			if *list_templates {
				// TODO
				Ok(())
			} else {
				crate::init::init(template, name, memory, disk)
			}
		}
		Commands::MakeUsb {
			output,
			confirm,
			include,
		} => {
			if Path::new(output).exists() {
				// TODO prompt
				//panic!();
			}

			Ok(())
		}
		Commands::Image { command } => match &command {
			ImageCommands::List {} => {
				let images = ImageLibrary::load()?;

				println!("Image Name      Image Size   Build Date                      Image ID     Description");
				for image in images {
					println!(
						"{:15} {:12} {:31} {:12} {}",
						image.metadata.config.name,
						image.size.bytes().to_string(),
						chrono::Utc
							.timestamp(image.metadata.timestamp as i64, 0)
							.to_rfc2822(),
						&image.id[0..12],
						image.metadata.config.description.unwrap_or("".to_string())
					);
				}
				Ok(())
			}
			ImageCommands::Info { image } => Ok(()),
			ImageCommands::Run { image } => Ok(()),
		},
		Commands::Write {
			image,
			output,
			confirm,
		} => {
			let image = ImageLibrary::find_by_id(image)?;

			if Path::new(output).exists() {
				// TODO prompt
				//panic!();
			}

			image.write(output)
		}
	}
}
