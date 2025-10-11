use crate::builder::Builder;
use crate::config::ConfigPath;
use std::process::ExitCode;
use tracing::{debug, error};
use validator::Validate;

pub fn run(cmd: super::Commands) -> ExitCode {
    match cmd.clone() {
        super::Commands::Build {
            record,
            debug,
            read_password,
            no_accel,
            output,
            path,
            ovmf_path,
        } => {
            let config_path = match ConfigPath::from_dir(path) {
                Some(p) => {
                    debug!("Loading config from {}", p);
                    p
                }
                _ => {
                    error!("Failed to find config file");
                    return ExitCode::FAILURE;
                }
            };

            // Load config from current directory
            let Ok(elements) = config_path.load() else {
                return ExitCode::FAILURE;
            };
            debug!("Loaded: {:#?}", &elements);

            let mut builder = Builder::new(elements);

            // Include the encryption password if provided
            if read_password {
                print!("Enter password: ");
                let mut password = String::new();
                std::io::stdin().read_line(&mut password).unwrap();
                // config.password = Some(password);
            } else if let Ok(_password) = std::env::var("GOLDBOOT_PASSWORD") {
                // Wipe out the value since we no longer need it
                unsafe {
                    std::env::set_var("GOLDBOOT_PASSWORD", "");
                }
                // config.password = Some(password);
            }

            // Fully verify config before proceeding
            match builder.validate() {
                Err(err) => {
                    error!(error = ?err, "Failed to validate config file");
                    return ExitCode::FAILURE;
                }
                _ => debug!("Validated config file"),
            };

            // Run the build finally
            match builder.run(cmd) {
                Err(err) => {
                    error!(error = ?err, "Failed to build image");
                    ExitCode::FAILURE
                }
                _ => ExitCode::SUCCESS,
            }
        }
        _ => panic!(),
    }
}
