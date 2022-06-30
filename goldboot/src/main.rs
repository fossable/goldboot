use crate::cmd::Commands;
use chrono::TimeZone;
use clap::{Parser, Subcommand};
use goldboot::{build::BuildJob, library::ImageLibrary, templates::TemplateBase, BuildConfig, *};
use log::debug;
use simple_error::bail;
use std::{collections::HashMap, env, error::Error, fs::File, path::Path};
use ubyte::ToByteUnit;
use validator::Validate;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct CommandLine {
	#[clap(subcommand)]
	command: Commands,
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
		Commands::Init => crate::cmd::init::run(cl.command),
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
	}
}
