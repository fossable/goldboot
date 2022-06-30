use console::Style;
use crate::cmd::Commands;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};

pub fn run(cmd: crate::cmd::Commands) -> Result<(), Box<dyn Error>> {

    match cmd {
        Commands::Registry { command } => match &command {
			RegistryCommands::Push { url } => todo!(),
			RegistryCommands::Pull { url } => todo!(),
		},
        _ => panic!(),
    }
}
