use crate::foundry::{Foundry, FoundryConfig};
use log::debug;
use std::error::Error;
use validator::Validate;

pub fn run(cmd: super::Commands) -> Result<(), Box<dyn Error>> {
    match cmd {
        super::Commands::Cast {
            record,
            debug,
            read_password,
            output,
            path,
        } => {
            let config_path =
                FoundryConfig::from_dir(path).ok_or(Err("No goldboot config found"))?;
            debug!("Loading config from {}", config_path);

            // Load config from current directory
            let mut foundry: Foundry = config_path.load()?;
            debug!("Loaded: {:#?}", &foundry);

            // Include the encryption password if provided
            if read_password {
                print!("Enter password: ");
                let mut password = String::new();
                std::io::stdin().read_line(&mut password)?;
                // config.password = Some(password);
            } else if let Ok(password) = std::env::var("GOLDBOOT_PASSWORD") {
                // Wipe out the value since we no longer need it
                std::env::set_var("GOLDBOOT_PASSWORD", "");
                // config.password = Some(password);
            }

            // Fully verify config before proceeding
            foundry.validate()?;

            // Run the build finally
            let mut job = BuildJob::new(config, record, debug);
            job.run(output.to_owned())
        }
        _ => panic!(),
    }
}
