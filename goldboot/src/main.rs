#[cfg(all(feature = "cli", feature = "uki"))]
compile_error!("features \"cli\" and \"uki\" are mutually exclusive");

#[cfg(feature = "cli")]
use clap::Parser;
#[cfg(feature = "cli")]
use goldboot::cli::cmd::Commands;
use std::os::unix::process::CommandExt;
use std::process::Command;
use std::process::ExitCode;
use tracing::debug;

#[cfg(feature = "cli")]
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
    if std::env::var("CI").is_ok() {
        return true;
    }
    if std::env::var("GOLDBOOT_DEBUG").is_ok() {
        return false;
    }
    true
}

pub fn main() -> ExitCode {
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .ok();

    // UKI mode: Run fullscreen GUI with automatic environment checks and reboot
    #[cfg(feature = "uki")]
    return uki_main();

    // CLI mode
    #[cfg(all(feature = "cli", not(feature = "uki")))]
    return cli_main();

    #[cfg(not(any(feature = "cli", feature = "uki")))]
    {
        eprintln!("No features enabled. Build with --features cli or --features uki");
        ExitCode::FAILURE
    }
}

#[cfg(all(feature = "cli", not(feature = "uki")))]
fn cli_main() -> ExitCode {
    // Parse command line options before we configure logging so we can set the
    // default level
    let command_line = CommandLine::parse();

    // Configure logging
    {
        let _default_filter = match &command_line.command {
            #[cfg(feature = "build")]
            Some(Commands::Build { debug, .. }) if *debug => "debug",
            _ => "info",
        };

        tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .init();
    }

    // Dispatch command
    match &command_line.command {
        #[cfg(feature = "build")]
        Some(Commands::Init { .. }) => goldboot::cli::cmd::init::run(command_line.command.unwrap()),
        #[cfg(feature = "build")]
        Some(Commands::Build { .. }) => {
            goldboot::cli::cmd::build::run(command_line.command.unwrap())
        }
        Some(Commands::Image { .. }) => {
            goldboot::cli::cmd::image::run(command_line.command.unwrap())
        }
        Some(Commands::Deploy { .. }) => {
            goldboot::cli::cmd::deploy::run(command_line.command.unwrap())
        }
        Some(Commands::Drift { .. }) => {
            goldboot::cli::cmd::drift::run(command_line.command.unwrap())
        }
        Some(Commands::Install { .. }) => {
            goldboot::cli::cmd::install::run(command_line.command.unwrap())
        }
        Some(Commands::Lsp) => {
            let rust_analyzer: roniker::RustAnalyzer = serde_json::from_str(include_str!(concat!(
                env!("OUT_DIR"),
                "/rust_analyzer.json"
            )))
            .expect("Failed to deserialize RustAnalyzer");

            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(roniker::serve(rust_analyzer, true));
            ExitCode::FAILURE
        }
        None => {
            #[cfg(feature = "gui")]
            {
                goldboot::gui::run_gui(command_line.fullscreen)
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
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("Initializing UKI boot mode");

    goldboot::gui::run_gui(true);

    debug!("Rebooting on GUI exit");

    let err = Command::new("reboot").exec();
    panic!("Failed to execute reboot: {}", err);
}
