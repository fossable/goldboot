use anyhow::bail;
use anyhow::Result;
use console::Style;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};
use goldboot_image::ImageArch;
use std::{error::Error, path::Path};
use strum::IntoEnumIterator;

use crate::foundry::{molds::ImageMold, Foundry, FoundryConfig};

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

pub fn run(cmd: super::Commands) -> Result<()> {
    match cmd {
        super::Commands::Init {
            name,
            mold,
            mimic_hardware,
        } => {
            let config_path =
                FoundryConfig::from_dir(".").unwrap_or(FoundryConfig::Ron("./goldboot.ron".into()));

            // Build a new default config that we'll override
            let mut foundry = Foundry::default();

            if mold.len() > 0 {
                // If a mold name was given, use the default
                if let Some(name) = name {
                    foundry.name = name;
                } else {
                    // Set name equal to directory name
                    if let Some(name) = std::env::current_dir()?.file_name() {
                        foundry.name = name.to_str().unwrap().to_string();
                    }
                }

                // Add default templates
                // for template_id in template {
                //     if let Some(id) = Template::iter()
                //         .filter(|id| id.to_string() == template_id)
                //         .next()
                //     {
                //         config.templates.push(id.default());
                //     } else {
                //         bail!("Template not found");
                //     }
                // }

                // Generate QEMU flags for this hardware
                //config.qemuargs = generate_qemuargs()?;
            } else {
                // If no mold was given, begin interactive config
                print_banner();

                let theme = ColorfulTheme {
                    values_style: Style::new().yellow().dim(),
                    ..ColorfulTheme::default()
                };

                println!("Get ready to create a new image configuration!");
                println!("(it can be further edited later)");
                println!();

                // Prompt image name
                foundry.name = Input::with_theme(&theme)
                    .with_prompt("Enter image name")
                    .default(
                        std::env::current_dir()?
                            .file_name()
                            .unwrap()
                            .to_str()
                            .unwrap()
                            .to_string(),
                    )
                    .interact()?;

                // Prompt image architecture
                {
                    let architectures: Vec<ImageArch> = ImageArch::iter().collect();
                    let choice_index = Select::with_theme(&theme)
                        .with_prompt("Choose image architecture")
                        .default(0)
                        .items(&architectures)
                        .interact()?;

                    foundry.arch = architectures[choice_index];
                }

                loop {
                    // Find molds suitable for the architecture
                    let molds: Vec<ImageMold> = ImageMold::iter()
                        .filter(|mold| mold.architectures().contains(&foundry.arch))
                        .filter(|mold| foundry.molds().len() == 0 || mold.alloy())
                        .collect();

                    let choice_index = Select::with_theme(&theme)
                        .with_prompt("Choose image mold")
                        .items(&molds)
                        .interact()?;

                    let mold = &molds[choice_index];
                    // foundry.templates.push(mold);

                    if !mold.alloy()
                        || !Confirm::with_theme(&theme)
                            .with_prompt("Do you want to add another OS for multibooting?")
                            .interact()?
                    {
                        break;
                    }
                }
            }

            // Finally write out the config
            config_path.write(&foundry)?;
            Ok(())
        }
        _ => panic!(),
    }
}
