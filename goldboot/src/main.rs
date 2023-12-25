use anyhow::Result;
use clap::Parser;
use goldboot::cli::cmd::Commands;
use std::env;

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

pub fn main() -> Result<()> {
    // Parse command line options before we configure logging so we can set the
    // default level
    let command_line = CommandLine::parse();

    // Configure logging
    {
        let default_filter = match &command_line.command {
            Commands::Cast {
                record: _,
                debug,
                read_password: _,
                output: _,
                path: _,
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
        Commands::Init { .. } => goldboot::cli::cmd::init::run(command_line.command),
        Commands::Cast { .. } => goldboot::cli::cmd::build::run(command_line.command),
        Commands::Image { .. } => goldboot::cli::cmd::image::run(command_line.command),
        Commands::Registry { .. } => goldboot::cli::cmd::registry::run(command_line.command),
        Commands::Write { .. } => goldboot::cli::cmd::write::run(command_line.command),
    }
}
