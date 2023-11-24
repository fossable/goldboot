use console::Style;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};
use log::debug;
use std::{error::Error, path::Path};
use validator::Validate;

use crate::foundry::Foundry;

pub fn run(cmd: super::Commands) -> Result<(), Box<dyn Error>> {
    match cmd {
        super::Commands::Build {
            record,
            debug,
            read_password,
            output,
            path,
        } => {
            let context_path = if let Some(path) = path.to_owned() {
                path.as_str()
            } else {
                if Path::new("./goldboot.yaml").exists() {
                    "./goldboot.yaml"
                } else if Path::new("./goldboot.yml").exists() {
                    "./goldboot.yml"
                } else {
                    todo!()
                }
            };
            debug!("Loading config from {}", config_path);

            // Load config from current directory
            let mut foundry: Foundry = ron::de::from_bytes(&std::fs::read(config_path)?)?;
            debug!("Loaded: {:#?}", &foundry);

            // Include the encryption password if provided
            if read_password {
                print!("Enter password: ");
                let mut password = String::new();
                std::io::stdin().read_line(&mut password)?;
                config.password = Some(password);
            } else if let Ok(password) = std::env::var("GOLDBOOT_PASSWORD") {
                // Wipe out the value since we no longer need it
                std::env::set_var("GOLDBOOT_PASSWORD", "");
                config.password = Some(password);
            }

            // Fully verify config before proceeding
            config.validate()?;

            // Run the build finally
            let mut job = BuildJob::new(config, record, debug);
            job.run(output.to_owned())
        }
        _ => panic!(),
    }
}
