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
	// Parse command line options before we configure logging so we can set the
	// default level
	let command_line = CommandLine::parse();

	// Configure logging
	{
		let default_filter = match &command_line.command {
			Commands::Build {
				record,
				debug,
				read_password,
				output,
				config,
			} => {
				if *debug {
					"debug"
				} else {
					"info"
				}
			}
			_ => "info",
		};

		env_logger::init_from_env(env_logger::Env::new().default_filter_or(default_filter));
	}

	// Dispatch command
	match &command_line.command {
		Commands::Init { .. } => crate::cmd::init::run(command_line.command),
		Commands::Build { .. } => crate::cmd::build::run(command_line.command),
		Commands::Image { .. } => crate::cmd::image::run(command_line.command),
		Commands::Registry { .. } => crate::cmd::registry::run(command_line.command),
		Commands::Write { .. } => crate::cmd::write::run(command_line.command),
	}
}
