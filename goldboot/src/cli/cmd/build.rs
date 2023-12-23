use crate::foundry::{Foundry, FoundryConfig};
use anyhow::anyhow;
use anyhow::Result;
use log::debug;
use validator::Validate;

pub fn run(cmd: super::Commands) -> Result<()> {
    match cmd {
        super::Commands::Cast {
            record,
            debug,
            read_password,
            output,
            path,
        } => {
            let config_path = FoundryConfig::from_dir(path.unwrap_or(".".to_string()))
                .ok_or_else(|| anyhow!("No config found"))?;
            debug!("Loading config from {}", config_path);

            // Load config from current directory
            let mut foundry: Foundry = config_path.load()?;
            foundry.debug = debug;
            foundry.record = record;
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
            foundry.run(output)?;
            Ok(())
        }
        _ => panic!(),
    }
}
