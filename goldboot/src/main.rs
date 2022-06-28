use chrono::TimeZone;
use clap::{Parser, Subcommand};
use colored::*;
use goldboot::{build::BuildJob, library::ImageLibrary, templates::TemplateBase, BuildConfig, *};
use log::debug;
use simple_error::bail;
use std::{collections::HashMap, env, error::Error, fs::File, path::Path};
use ubyte::ToByteUnit;
use validator::Validate;

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
		/// Save a screenshot to ./screenshots after each boot command for
		/// debugging
		#[clap(long, takes_value = false)]
		record: bool,

		/// Insert a breakpoint after each boot command
		#[clap(long, takes_value = false)]
		debug: bool,

		/// Read the encryption password from STDIN
		#[clap(long, takes_value = false)]
		read_password: bool,

		/// The optional output destination (defaults to image library)
		#[clap(long)]
		output: Option<String>,

		/// The config file path (default: ./goldboot.json)
		#[clap(long)]
		config: Option<String>,
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

		/// The target architecture
		#[clap(long)]
		arch: Option<String>,

		/// The amount of memory the image can access
		#[clap(long)]
		memory: Option<String>,

		/// The amount of storage the image can access
		#[clap(long)]
		disk: Option<String>,

		/// Attempt to copy the configuration of the current hardware as closely
		/// as possible
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

		/// The architecture of the image to download
		#[clap(long)]
		arch: Option<String>,
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
		Commands::Build {
			record,
			debug,
			read_password,
			output,
			config,
		} => {
			print_banner();

			let config_path = if let Some(path) = config.to_owned() {
				path
			} else {
				String::from("./goldboot.json")
			};
			debug!("Loading config from {}", config_path);

			// Load build config from current directory
			let mut config: BuildConfig = serde_json::from_slice(&std::fs::read(config_path)?)?;
			debug!("Loaded: {:#?}", &config);

			// Include the encryption password if provided
			if *read_password {
				print!("Enter password: ");
				let mut password = String::new();
				std::io::stdin().read_line(&mut password)?;
				config.password = Some(password);
			} else if let Ok(password) = env::var("GOLDBOOT_PASSWORD") {
				// Wipe out the value since we no longer need it
				env::set_var("GOLDBOOT_PASSWORD", "");
				config.password = Some(password);
			}

			// Fully verify config before proceeding
			config.validate()?;

			// Run the build finally
			let mut job = BuildJob::new(config, *record, *debug);
			job.run(output.to_owned())
		}
		Commands::Registry { command } => match &command {
			RegistryCommands::Push { url } => todo!(),
			RegistryCommands::Pull { url } => todo!(),
		},
		Commands::Init {
			name,
			template,
			arch,
			memory,
			disk,
			list_templates,
			mimic_hardware,
		} => {
			if *list_templates {
				// TODO
				return Ok(());
			}

			let config_path = Path::new("goldboot.json");

			if config_path.exists() {
				bail!("This directory has already been initialized. Delete goldboot.json to reinitialize.");
			}

			if template.len() == 0 {
				bail!("Specify at least one template with --template");
			}

			// Create a new config to be filled in according to the given arguments
			let mut config = BuildConfig::default();

			if let Some(name) = name {
				config.name = name.to_string();
			} else {
				// Set name equal to directory name
				if let Some(name) = env::current_dir()?.file_name() {
					config.name = name.to_str().unwrap().to_string();
				}
			}

			// Generate QEMU flags for this hardware
			//config.qemuargs = generate_qemuargs()?;

			// Set architecture if given
			if let Some(arch) = arch {
				config.arch = arch.to_owned().try_into()?;
			}

			// Run template-specific initialization
			let mut default_templates = Vec::new();
			for t in template {
				let t: TemplateBase =
					serde_json::from_str(format!("{{\"base\": \"{}\"}}", &t).as_str())?;
				default_templates.push(t.get_default_template()?);
			}
			config.templates = default_templates;

			// Finally write out the config
			std::fs::write(config_path, serde_json::to_string_pretty(&config)?)?;
			Ok(())
		}
		Commands::MakeUsb {
			output,
			confirm,
			include,
			arch,
		} => {
			if Path::new(output).exists() && !*confirm {
				// Prompt to continue
				print!("Confirm? [Y/N]");
				let mut answer = String::new();
				std::io::stdin().read_line(&mut answer)?;

				match answer.as_str() {
					"y" => {}
					"Y" => {}
					_ => std::process::exit(0),
				}
			}

			// Find latest release
			let rs: HashMap<String, serde_json::Value> = reqwest::blocking::Client::new()
				.get("https://github.com/goldboot/goldboot/releases/latest")
				.header("Accept", "application/json")
				.send()?
				.json()?;

			if let Some(version) = rs.get("tag_name") {
				let version = version.as_str().unwrap();

				let arch = arch.clone().unwrap_or("amd64".to_string());

				// Download latest release to library
				let image = ImageLibrary::download(format!("https://github.com/goldboot/goldboot/releases/download/{version}/goldboot-linux-{arch}.gb"))?;

				// Write image to device
				image.write(output)
			} else {
				panic!();
			}
		}
		Commands::Image { command } => match &command {
			ImageCommands::List {} => {
				let images = ImageLibrary::load()?;

				println!("Image Name      Image Size   Build Date                      Image ID     Description");
				for image in images {
					println!(
						"{:15} {:12} {:31} {:12} {}",
						std::str::from_utf8(&image.primary_header.name)?,
						image.primary_header.size.bytes().to_string(),
						chrono::Utc
							.timestamp(image.primary_header.timestamp as i64, 0)
							.to_rfc2822(),
						&image.id[0..12],
						"TODO",
					);
				}
				Ok(())
			}
			ImageCommands::Info { image } => {
				let image = ImageLibrary::find_by_id(image)?;
				// TODO
				Ok(())
			}
			ImageCommands::Run { image } => Ok(()),
		},
		Commands::Write {
			image,
			output,
			confirm,
		} => {
			let image = ImageLibrary::find_by_id(image)?;

			if Path::new(output).exists() && !*confirm {
				// Prompt to continue
				print!("Confirm? [Y/N]");
				let mut answer = String::new();
				std::io::stdin().read_line(&mut answer)?;

				match answer.as_str() {
					"y" => {}
					"Y" => {}
					_ => std::process::exit(0),
				}
			}

			image.write(output)
		}
	}
}
