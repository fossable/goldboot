use console::Style;
use dialoguer::{theme::ColorfulTheme, Input};
use std::process::ExitCode;

use super::RegistryCommands;

pub fn run(cmd: super::Commands) -> ExitCode {
    let theme = ColorfulTheme {
        values_style: Style::new().yellow().dim(),
        ..ColorfulTheme::default()
    };

    match cmd {
        super::Commands::Registry { command } => match &command {
            RegistryCommands::Push { url: _ } => todo!(),
            RegistryCommands::Pull { url: _ } => todo!(),
            RegistryCommands::Login {} => {
                // Prompt registry URL
                let _registry_url: String = Input::with_theme(&theme)
                    .with_prompt("Enter registry URL")
                    .interact()
                    .unwrap();

                // Prompt registry token
                let _registry_token: String = Input::with_theme(&theme)
                    .with_prompt("Enter registry token")
                    .interact()
                    .unwrap();

                // Prompt token passphrase
                let _token_passphrase: String = Input::with_theme(&theme)
                    .with_prompt(
                        "Enter a passphrase to encrypt the token or nothing to store plaintext",
                    )
                    .interact()
                    .unwrap();
                ExitCode::SUCCESS
            }
        },
        _ => panic!(),
    }
}
