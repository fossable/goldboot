use crate::foundry::{Foundry, FoundryConfigPath};
use std::process::ExitCode;
use tracing::debug;
use tracing::error;
use validator::Validate;

pub fn run(cmd: super::Commands) -> ExitCode {
    match cmd {
        super::Commands::Cast {
            record,
            debug,
            read_password,
            no_accel,
            output,
            path,
        } => {
            let config_path = match FoundryConfigPath::from_dir(path) {
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
            let mut foundry: Foundry = config_path.load().unwrap();
            foundry.debug = debug;
            foundry.record = record;
            debug!("Loaded: {:#?}", &foundry);

            // Include the encryption password if provided
            if read_password {
                print!("Enter password: ");
                let mut password = String::new();
                std::io::stdin().read_line(&mut password).unwrap();
                // config.password = Some(password);
            } else if let Ok(_password) = std::env::var("GOLDBOOT_PASSWORD") {
                // Wipe out the value since we no longer need it
                std::env::set_var("GOLDBOOT_PASSWORD", "");
                // config.password = Some(password);
            }

            // Fully verify config before proceeding
            match foundry.validate() {
                Err(err) => {
                    error!("Failed to validate config file");
                    return ExitCode::FAILURE;
                }
                _ => debug!("Validated config file"),
            };

            // Run the build finally
            match foundry.run(output) {
                Err(err) => {
                    error!(error = %err, "Failed to cast image");
                    ExitCode::FAILURE
                }
                _ => ExitCode::SUCCESS,
            }
        }
        _ => panic!(),
    }
}
