use clap::Parser;
use goldboot::cli::cmd::Commands;
use std::{env, process::ExitCode};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct CommandLine {
    #[clap(subcommand)]
    command: Option<Commands>,

    /// Run the GUI in fullscreen mode
    #[cfg(feature = "gui")]
    #[clap(long, num_args = 0)]
    fullscreen: bool,
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

pub fn main() -> ExitCode {
    // Parse command line options before we configure logging so we can set the
    // default level
    let command_line = CommandLine::parse();

    // Configure logging
    {
        let default_filter = match &command_line.command {
            Some(Commands::Cast {
                record: _,
                debug,
                read_password: _,
                no_accel: _,
                output: _,
                path: _,
            }) => {
                if *debug {
                    "debug"
                } else {
                    "info"
                }
            }
            _ => "info",
        };

        tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .init();
    }

    // Dispatch command
    match &command_line.command {
        Some(Commands::Init { .. }) => goldboot::cli::cmd::init::run(command_line.command.unwrap()),
        Some(Commands::Cast { .. }) => goldboot::cli::cmd::cast::run(command_line.command.unwrap()),
        Some(Commands::Image { .. }) => {
            goldboot::cli::cmd::image::run(command_line.command.unwrap())
        }
        Some(Commands::Registry { .. }) => {
            goldboot::cli::cmd::registry::run(command_line.command.unwrap())
        }
        Some(Commands::Write { .. }) => {
            goldboot::cli::cmd::write::run(command_line.command.unwrap())
        }
        #[cfg(feature = "gui")]
        None => goldboot::gui::load_gui(command_line.fullscreen),
        #[cfg(not(feature = "gui"))]
        None => panic!("Not supported"),
    }
}
