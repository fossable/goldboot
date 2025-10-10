use clap::ValueEnum;
use console::Style;
use dialoguer::{Confirm, Input, Password, Select, theme::ColorfulTheme};
use goldboot_image::ImageArch;
use std::process::ExitCode;
use strum::IntoEnumIterator;
use tracing::{error, info};

use crate::{
    builder::{
        Builder,
        os::{DefaultSource, Os},
    },
    cli::prompt::Prompt,
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
            name,
            os,
            format,
            size,
            mimic_hardware: _,
        } => {
            let mut config_path = ConfigPath::from_dir(".").unwrap_or(format);

            // Build a new default config that we'll override
            let mut builder = Builder::default();

            // Use size from command line if given
            if let Some(size) = size {
                builder.size = size;
            }

            if os.len() > 0 {
                // If an OS was given, use the default
                if let Some(name) = name {
                    builder.name = name;
                } else {
                    // Set name equal to directory name
                    if let Some(name) = std::env::current_dir().unwrap().file_name() {
                        builder.name = name.to_str().unwrap().to_string();
                    }
                }

                for m in os {
                    if let Ok(source) = m.default_source(builder.arch) {
                        builder.alloy.push(Element {
                            source,
                            os: m,
                            fabricators: None,
                            pref_size: None,
                        });
                    } else {
                        return ExitCode::FAILURE;
                    }
                }

                // Generate QEMU flags for this hardware
                //config.qemuargs = generate_qemuargs()?;
            } else {
                // If no OS was given, begin interactive config
                print_banner();

                let theme = theme();

                println!("Get ready to create a new image configuration!");
                println!("(it can be further edited later)");
                println!();

                // Prompt config format
                {
                    let formats: &[ConfigPath] = ConfigPath::value_variants();
                    let choice_index = Select::with_theme(&theme)
                        .with_prompt("Config format?")
                        .default(0)
                        .items(formats.iter())
                        .interact()
                        .unwrap();
                    config_path = formats[choice_index].clone();
                }

                // Prompt image name
                builder.name = Input::with_theme(&theme)
                    .with_prompt("Image name?")
                    .default(
                        std::env::current_dir()
                            .unwrap()
                            .file_name()
                            .unwrap()
                            .to_str()
                            .unwrap()
                            .to_string(),
                    )
                    .interact()
                    .unwrap();

                // Prompt encryption password
                if Confirm::with_theme(&theme)
                    .with_prompt("Encrypt image at rest?")
                    .interact()
                    .unwrap()
                {
                    builder.password = Some(
                        Password::with_theme(&theme)
                            .with_prompt("Encryption passphrase?")
                            .interact()
                            .unwrap(),
                    );
                }

                // Prompt image architecture
                {
                    let architectures: Vec<ImageArch> = ImageArch::iter().collect();
                    let choice_index = Select::with_theme(&theme)
                        .with_prompt("Image architecture?")
                        .default(0)
                        .items(&architectures)
                        .interact()
                        .unwrap();

                    builder.arch = architectures[choice_index];
                }

                // Prompt OS
                loop {
                    // Find operating systems suitable for the architecture
                    let mut supported_os: Vec<Os> = Os::iter()
                        .filter(|os| os.architectures().contains(&builder.arch))
                        .filter(|os| builder.alloy.len() == 0 || os.alloy())
                        .collect();

                    let choice_index = Select::with_theme(&theme)
                        .with_prompt("Operating system?")
                        .items(&supported_os)
                        .interact()
                        .unwrap();

                    let os = &mut supported_os[choice_index];

                    if Confirm::with_theme(&theme)
                        .with_prompt("Edit OS configuration?")
                        .interact()
                        .unwrap()
                    {
                        // TODO show some kind of banner
                        os.prompt(&builder).unwrap();
                    }

                    if let Ok(source) = os.default_source(builder.arch) {
                        builder.alloy.push(Element {
                            source,
                            os: os.to_owned(),
                            fabricators: None,
                            pref_size: None,
                        });
                    } else {
                        return ExitCode::FAILURE;
                    }

                    if !os.alloy()
                        || !Confirm::with_theme(&theme)
                            .with_prompt("Create an alloy image (multiboot)?")
                            .interact()
                            .unwrap()
                    {
                        break;
                    }
                }

                // Prompt size
                builder.size = Input::with_theme(&theme)
                    .with_prompt("Image size?")
                    .default("28GiB".to_string())
                    .interact()
                    .unwrap();
            }

            // Finally write out the config
            match config_path.write(&builder) {
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
