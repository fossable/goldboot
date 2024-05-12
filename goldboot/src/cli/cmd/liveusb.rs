use console::Style;
use dialoguer::{theme::ColorfulTheme, Confirm};
use goldboot_image::ImageHandle;
use std::{path::Path, process::ExitCode};
use tracing::error;

use crate::{cli::progress::ProgressBar, library::ImageLibrary};

pub fn run(cmd: super::Commands) -> ExitCode {
    match cmd {
        super::Commands::Liveusb {
            dest,
            include,
            confirm,
        } => {
            let theme = ColorfulTheme {
                values_style: Style::new().yellow().dim(),
                ..ColorfulTheme::default()
            };

            if !Path::new(&dest).exists() {
                return ExitCode::FAILURE;
            }

            if !confirm {
                if !Confirm::with_theme(&theme)
                    .with_prompt("Do you want to continue?")
                    .interact()
                    .unwrap()
                {
                    std::process::exit(0);
                }
            }

            ExitCode::SUCCESS
        }
        _ => panic!(),
    }
}
