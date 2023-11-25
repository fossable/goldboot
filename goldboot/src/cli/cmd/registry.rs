use console::Style;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};
use std::error::Error;

use super::RegistryCommands;

pub fn run(cmd: super::Commands) -> Result<(), Box<dyn Error>> {
    let theme = ColorfulTheme {
        values_style: Style::new().yellow().dim(),
        ..ColorfulTheme::default()
    };

    match cmd {
        super::Commands::Registry { command } => match &command {
            RegistryCommands::Push { url } => todo!(),
            RegistryCommands::Pull { url } => todo!(),
            RegistryCommands::Login {} => {
                // Prompt registry URL
                let registry_url: String = Input::with_theme(&theme)
                    .with_prompt("Enter registry URL")
                    .interact()?;

                // Prompt registry token
                let registry_token: String = Input::with_theme(&theme)
                    .with_prompt("Enter registry token")
                    .interact()?;

                // Prompt token passphrase
                let token_passphrase: String = Input::with_theme(&theme)
                    .with_prompt(
                        "Enter a passphrase to encrypt the token or nothing to store plaintext",
                    )
                    .interact()?;
                Ok(())
            }
        },
        _ => panic!(),
    }
}
