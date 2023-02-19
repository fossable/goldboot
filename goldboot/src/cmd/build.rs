use crate::{
    build::{BuildConfig, BuildJob},
    cmd::Commands,
};
use console::Style;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};
use log::debug;
use std::{error::Error, path::Path};
use validator::Validate;

pub fn run(cmd: crate::cmd::Commands) -> Result<(), Box<dyn Error>> {
    match cmd {
        Commands::Build {
            record,
            debug,
            read_password,
            output,
            config,
        } => {
            let config_path = if let Some(path) = config.to_owned() {
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

            // Load build config from current directory
            let mut config: BuildConfig = serde_yaml::from_slice(&std::fs::read(config_path)?)?;
            debug!("Loaded: {:#?}", &config);

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
