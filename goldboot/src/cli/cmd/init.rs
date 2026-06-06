use console::Style;
use dialoguer::{Confirm, Input, Select, theme::ColorfulTheme};
use goldboot_image::{ImageArch, validate_ref_segment};
use std::path::PathBuf;
use std::process::ExitCode;
use strum::IntoEnumIterator;
use tracing::{error, info};

use crate::builder::{
    Builder,
    config::ConfigPath,
    os::{OsConfig, os_iter},
};

fn print_banner() {
    if console::colors_enabled() {
        let style = Style::new().yellow();

        println!();
        for line in fossable::goldboot_word() {
            println!("  {}", style.apply_to(line));
        }
        println!();
    }
}

pub fn theme() -> ColorfulTheme {
    ColorfulTheme {
        values_style: Style::new().yellow().dim(),
        ..ColorfulTheme::default()
    }
}

pub fn run(cmd: super::Commands) -> ExitCode {
    match cmd {
        super::Commands::Init {
            name,
            os,
            mimic_hardware: _,
        } => {
            let cwd = std::env::current_dir().unwrap();

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

            let initial_name = name.clone().unwrap_or_else(|| "image".to_string());
            let mut builder = Builder::new(initial_name, initial_elements, cwd.clone());
            let mut name_provided = name.is_some();

            if builder.elements.is_empty() {
                print_banner();

                let theme = theme();

                println!("Get ready to create a new image configuration!");
                println!("(it can be further edited later)");
                println!();

                builder.name = loop {
                    let candidate: String = Input::with_theme(&theme)
                        .with_prompt("Image name?")
                        .with_initial_text(&builder.name)
                        .interact_text()
                        .unwrap();
                    if let Err(e) = validate_ref_segment(&candidate) {
                        eprintln!("invalid name: {e}");
                        continue;
                    }
                    break candidate;
                };
                name_provided = true;

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

                loop {
                    let supported_descriptors: Vec<&crate::builder::os::OsDescriptor> = os_iter()
                        .filter(|d| d.architectures.contains(&arch))
                        .filter(|_d| builder.elements.is_empty())
                        .collect();

                    let os_names: Vec<&str> =
                        supported_descriptors.iter().map(|d| d.name).collect();

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

            // Pick filename based on whether a name was provided/chosen.
            let target_path: PathBuf = if name_provided {
                cwd.join(format!("{}.goldboot.ron", builder.name))
            } else {
                cwd.join("goldboot.ron")
            };

            // Refuse to overwrite an existing config.
            if target_path.exists() {
                error!(
                    "Refusing to overwrite existing config at {}",
                    target_path.display()
                );
                return ExitCode::FAILURE;
            }

            let config_path = ConfigPath::with_path(target_path);
            let elements = std::mem::take(&mut builder.elements);
            match config_path.write(&elements) {
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
