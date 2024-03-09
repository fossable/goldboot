use console::Style;
use dialoguer::{theme::ColorfulTheme, Confirm};
use std::{path::Path, process::ExitCode};
use tracing::error;

use crate::{cli::progress::ProgressBar, library::ImageLibrary};

pub fn run(cmd: super::Commands) -> ExitCode {
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

            let image = ImageLibrary::find_by_id(&image).unwrap();

            if Path::new(&output).exists() && !confirm {
                if !Confirm::with_theme(&theme)
                    .with_prompt("Do you want to continue?")
                    .interact()
                    .unwrap()
                {
                    std::process::exit(0);
                }
            }

            // TODO special case for GBL; select images to include

            match image.write(output, ProgressBar::Write.new_empty()) {
                Err(err) => {
                    error!("Failed to write image");
                    ExitCode::FAILURE
                }
                _ => ExitCode::SUCCESS,
            }
        }
        _ => panic!(),
    }
}
