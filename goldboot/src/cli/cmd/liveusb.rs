use console::Style;
use dialoguer::{Confirm, theme::ColorfulTheme};
use std::{path::Path, process::ExitCode};
use tracing::error;

use crate::{cli::progress::ProgressBar, builder::os::Os, library::ImageLibrary};

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

            // Load from library or download
            let mut image_handles = match ImageLibrary::find_by_os("Goldboot") {
                Ok(image_handles) => image_handles,
                Err(_) => return ExitCode::FAILURE,
            };

            // TODO prompt password
            if image_handles[0].load(None).is_err() {
                return ExitCode::FAILURE;
            }

            if !confirm {
                if !Confirm::with_theme(&theme)
                    .with_prompt(format!("Do you want to overwrite: {}?", dest))
                    .interact()
                    .unwrap()
                {
                    return ExitCode::FAILURE;
                }
            }

            match image_handles[0].write(dest, ProgressBar::Write.new_empty()) {
                Err(err) => {
                    error!(error = ?err, "Failed to write image");
                    ExitCode::FAILURE
                }
                _ => ExitCode::SUCCESS,
            }
        }
        _ => panic!(),
    }
}
