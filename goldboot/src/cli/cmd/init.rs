use console::Style;
use dialoguer::{Confirm, Select, theme::ColorfulTheme};
use goldboot_image::ImageArch;
use std::process::ExitCode;
use strum::IntoEnumIterator;
use tracing::{error, info};

use crate::{
    builder::{Builder, os::{OsConfig, os_iter}},
    config::ConfigPath,
};

fn print_banner() {
    if console::colors_enabled() {
        let style = Style::new().yellow();

        println!("{}", "");
        for line in fossable::goldboot_word() {
            println!("  {}", style.apply_to(line));
        }
        println!("{}", "");
    }
}

/// Get the current theme for prompts.
pub fn theme() -> ColorfulTheme {
    ColorfulTheme {
        values_style: Style::new().yellow().dim(),
        ..ColorfulTheme::default()
    }
}

pub fn run(cmd: super::Commands) -> ExitCode {
    match cmd {
        super::Commands::Init {
            name: _,
            os,
            mimic_hardware: _,
        } => {
            let config_path = ConfigPath::from_dir(".").unwrap_or_default();

            // If OS was specified on the command line, resolve it to an OsConfig
            let initial_elements: Vec<OsConfig> = if let Some(os_names) = os {
                os_names
                    .into_iter()
                    .filter_map(|name| {
                        os_iter()
                            .find(|d| d.name == name)
                            .map(|d| OsConfig((d.default)()))
                    })
                    .collect()
            } else {
                Vec::new()
            };

            let mut builder = Builder::new(initial_elements);

            if builder.elements.is_empty() {
                // If no OS was given, begin interactive config
                print_banner();

                let theme = theme();

                println!("Get ready to create a new image configuration!");
                println!("(it can be further edited later)");
                println!();

                // Prompt image architecture
                let arch = {
                    let architectures: Vec<ImageArch> = ImageArch::iter().collect();
                    let choice_index = Select::with_theme(&theme)
                        .with_prompt("Image architecture?")
                        .default(0)
                        .items(&architectures)
                        .interact()
                        .unwrap();

                    architectures[choice_index]
                };

                // Prompt OS
                loop {
                    // Find operating systems suitable for the architecture
                    let supported_descriptors: Vec<&crate::builder::os::OsDescriptor> = os_iter()
                        .filter(|d| d.architectures.contains(&arch))
                        .filter(|_d| builder.elements.is_empty()) // alloy: always false for now
                        .collect();

                    let os_names: Vec<&str> = supported_descriptors.iter().map(|d| d.name).collect();

                    let choice_index = Select::with_theme(&theme)
                        .with_prompt("Operating system?")
                        .items(&os_names)
                        .interact()
                        .unwrap();

                    let descriptor = supported_descriptors[choice_index];
                    let mut os_config = OsConfig((descriptor.default)());

                    if Confirm::with_theme(&theme)
                        .with_prompt("Edit OS configuration?")
                        .interact()
                        .unwrap()
                    {
                        os_config.0.prompt(&builder).unwrap();
                    }

                    let alloy = os_config.0.os_alloy();
                    builder.elements.push(os_config);

                    if !alloy
                        || !Confirm::with_theme(&theme)
                            .with_prompt("Create an alloy image (multiboot)?")
                            .interact()
                            .unwrap()
                    {
                        break;
                    }
                }
            }

            // Finally write out the config
            match config_path.write(&builder.elements) {
                Err(err) => {
                    error!(error = ?err, "Failed to write config file");
                    ExitCode::FAILURE
                }
                _ => {
                    info!(path = %config_path, "Wrote goldboot config successfully");
                    ExitCode::SUCCESS
                }
            }
        }
        _ => panic!(),
    }
}
