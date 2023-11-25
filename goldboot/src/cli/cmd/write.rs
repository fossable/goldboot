use console::Style;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};
use std::{error::Error, path::Path};

pub fn run(cmd: super::Commands) -> Result<(), Box<dyn Error>> {
    match cmd {
        super::Commands::Write {
            image,
            output,
            confirm,
        } => {
            let theme = ColorfulTheme {
                values_style: Style::new().yellow().dim(),
                ..ColorfulTheme::default()
            };

            let image = ImageLibrary::find_by_id(&image)?;

            if Path::new(&output).exists() && !confirm {
                if !Confirm::with_theme(&theme)
                    .with_prompt("Do you want to continue?")
                    .interact()?
                {
                    std::process::exit(0);
                }
            }

            // TODO special case for GBL; select images to include

            image.write(output)
        }
        _ => panic!(),
    }
}
