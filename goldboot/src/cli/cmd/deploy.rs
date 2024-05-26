use console::Style;
use dialoguer::{theme::ColorfulTheme, Confirm};
use goldboot_image::ImageHandle;
use std::{path::Path, process::ExitCode};
use tracing::error;

use crate::{cli::progress::ProgressBar, library::ImageLibrary};

pub fn run(cmd: super::Commands) -> ExitCode {
    match cmd {
        super::Commands::Deploy {
            image,
            output,
            confirm,
        } => {
            let theme = ColorfulTheme {
                values_style: Style::new().yellow().dim(),
                ..ColorfulTheme::default()
            };

            let mut image_handle = if Path::new(&image).exists() {
                match ImageHandle::open(&image) {
                    Ok(image_handle) => image_handle,
                    Err(_) => return ExitCode::FAILURE,
                }
            } else {
                match ImageLibrary::find_by_id(&image) {
                    Ok(image_handle) => image_handle,
                    Err(_) => return ExitCode::FAILURE,
                }
            };
            if image_handle.load(None).is_err() {
                return ExitCode::FAILURE;
            }

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

            match image_handle.write(output, ProgressBar::Write.new_empty()) {
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
