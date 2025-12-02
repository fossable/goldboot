use clap::Parser;
use goldboot::cli::cmd::Commands;
use std::process::ExitCode;

#[cfg(not(feature = "uki"))]
use std::env;

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
    // UKI mode: Run fullscreen GUI with automatic environment checks and reboot
    #[cfg(feature = "uki")]
    return uki_main();

    // Parse command line options before we configure logging so we can set the
    // default level
    #[cfg(not(feature = "uki"))]
    let command_line = CommandLine::parse();

    // Configure logging
    #[cfg(not(feature = "uki"))]
    {
        let _default_filter = match &command_line.command {
            Some(Commands::Build { debug, .. }) => {
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
    #[cfg(not(feature = "uki"))]
    match &command_line.command {
        Some(Commands::Init { .. }) => goldboot::cli::cmd::init::run(command_line.command.unwrap()),
        Some(Commands::Build { .. }) => {
            goldboot::cli::cmd::build::run(command_line.command.unwrap())
        }
        Some(Commands::Image { .. }) => {
            goldboot::cli::cmd::image::run(command_line.command.unwrap())
        }
        Some(Commands::Registry { .. }) => {
            goldboot::cli::cmd::registry::run(command_line.command.unwrap())
        }
        Some(Commands::Deploy { .. }) => {
            goldboot::cli::cmd::deploy::run(command_line.command.unwrap())
        }
        Some(Commands::Liveusb { .. }) => {
            goldboot::cli::cmd::liveusb::run(command_line.command.unwrap())
        }
        None => {
            #[cfg(feature = "gui")]
            {
                return goldboot::gui::run_gui(command_line.fullscreen);
            }

            #[cfg(not(feature = "gui"))]
            {
                eprintln!("No command specified. Use --help for usage information.");
                eprintln!("Note: GUI requires building with --features gui");
                return ExitCode::FAILURE;
            }
        }
    }
}

#[cfg(feature = "uki")]
fn uki_main() -> ExitCode {
    use tracing::info;

    // Initialize logging for UKI mode
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    info!("Starting goldboot in UKI mode");

    // Check environment
    if let Err(e) = check_uki_environment() {
        eprintln!("Environment check failed: {}", e);
        return ExitCode::FAILURE;
    }

    // Run GUI in fullscreen mode
    let result = goldboot::gui::run_gui(true);

    // After GUI exits, reboot the system
    info!("GUI exited, initiating system reboot");
    if let Err(e) = reboot_system() {
        eprintln!("Failed to reboot system: {}", e);
        return ExitCode::FAILURE;
    }

    result
}

#[cfg(feature = "uki")]
fn check_uki_environment() -> Result<(), String> {
    // Verify we have access to block devices
    if !std::path::Path::new("/sys/class/block").exists() {
        return Err("Block device sysfs not available".to_string());
    }

    // Verify the image library directory exists
    let lib_path = std::path::Path::new("/var/lib/goldboot/images");
    if !lib_path.exists() {
        std::fs::create_dir_all(lib_path)
            .map_err(|e| format!("Failed to create library directory: {}", e))?;
    }

    Ok(())
}

#[cfg(feature = "uki")]
fn reboot_system() -> Result<(), String> {
    use std::process::Command;

    // Try systemctl first (if systemd is available)
    let result = Command::new("systemctl")
        .arg("reboot")
        .status();

    if result.is_ok() {
        return Ok(());
    }

    // Fallback to direct reboot command
    Command::new("reboot")
        .status()
        .map_err(|e| format!("Failed to execute reboot: {}", e))?;

    Ok(())
}
