use crate::{
    build::BuildConfig,
    cmd::Commands,
    templates::{Template, TemplateMetadata},
    Architecture,
};
use console::Style;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};
use simple_error::bail;
use std::{error::Error, path::Path};
use strum::IntoEnumIterator;

#[rustfmt::skip]
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

pub fn run(cmd: crate::cmd::Commands) -> Result<(), Box<dyn Error>> {
    match cmd {
        Commands::Init {
            name,
            template,
            mimic_hardware,
        } => {
            let config_path = Path::new("goldboot.yml");

            if config_path.exists() {
                bail!("This directory has already been initialized. Delete goldboot.yml to reinitialize.");
            }

            // Build a new default config that we'll override
            let mut config = BuildConfig::default();

            if template.len() > 0 {
                if let Some(name) = name {
                    config.name = name;
                } else {
                    // Set name equal to directory name
                    if let Some(name) = std::env::current_dir()?.file_name() {
                        config.name = name.to_str().unwrap().to_string();
                    }
                }

                // Add default templates
                for template_id in template {
                    if let Some(id) = Template::iter()
                        .filter(|id| id.to_string() == template_id)
                        .next()
                    {
                        config.templates.push(id.default());
                    } else {
                        bail!("Template not found");
                    }
                }

            // Generate QEMU flags for this hardware
            //config.qemuargs = generate_qemuargs()?;
            } else {
                // Begin interactive config
                print_banner();

                let theme = ColorfulTheme {
                    values_style: Style::new().yellow().dim(),
                    ..ColorfulTheme::default()
                };

                println!("Get ready to build a new image configuration!");
                println!("(it can be further edited later)");
                println!();

                // Prompt image name
                config.name = Input::with_theme(&theme)
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
                    let architectures: Vec<Architecture> = Architecture::iter().collect();
                    let arch_index = Select::with_theme(&theme)
                        .with_prompt("Choose image architecture")
                        .default(0)
                        .items(&architectures)
                        .interact()?;

                    config.arch = architectures[arch_index];
                }

                loop {
                    // Find templates suitable for the architecture
                    let templates: Vec<TemplateMetadata> = TemplateMetadata::all()
                        .into_iter()
                        .filter(|metadata| metadata.architectures.contains(&config.arch))
                        .filter(|metadata| config.templates.len() == 0 || metadata.multiboot)
                        .collect();

                    let template_index = Select::with_theme(&theme)
                        .with_prompt("Choose image template")
                        .items(&templates)
                        .interact()?;

                    let template_metadata = &templates[template_index];
                    config.templates.push(template_metadata.default());

                    if !template_metadata.multiboot
                        || !Confirm::with_theme(&theme)
                            .with_prompt("Do you want to add another OS for multibooting?")
                            .interact()?
                    {
                        break;
                    }
                }
            }

            // Finally write out the config
            std::fs::write(config_path, serde_yaml::to_string(&config)?)?;
            Ok(())
        }
        _ => panic!(),
    }
}
