use crate::{
	cmd::Commands,
	templates::{
		alpine_linux::AlpineLinuxTemplate, arch_linux::ArchLinuxTemplate, Promptable, Template,
		TemplateBase,
	},
	Architecture, BuildConfig,
};
use console::Style;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};
use simple_error::bail;
use std::{error::Error, path::Path};

#[rustfmt::skip]
fn print_banner() {
    if console::colors_enabled() {
        let style = Style::new().yellow();

        println!("{}", "");
        println!("  {}", style.apply_to("　　　　　　　　⬛　　　⬛　⬛　　　　　　　　　　　⬛　"));
        println!("  {}", style.apply_to("　　　　　　　　⬛　　　⬛　⬛　　　　　　　　　　　⬛⬛"));
        println!("  {}", style.apply_to("⬛⬛⬛　⬛⬛⬛　⬛　⬛⬛⬛　⬛⬛⬛　⬛⬛⬛　⬛⬛⬛　⬛　"));
        println!("  {}", style.apply_to("⬛　⬛　⬛　⬛　⬛　⬛　⬛　⬛　⬛　⬛　⬛　⬛　⬛　⬛　"));
        println!("  {}", style.apply_to("⬛⬛⬛　⬛⬛⬛　⬛　⬛⬛⬛　⬛⬛⬛　⬛⬛⬛　⬛⬛⬛　⬛⬛"));
        println!("  {}", style.apply_to("　　⬛　　　　　　　　　　　　　　　　　　　　　　　　　"));
        println!("  {}", style.apply_to("⬛⬛⬛　　　　　　　　　　　　　　　　　　　　　　　　　"));
        println!("{}", "");
    }
}

pub fn interactive_config() -> Result<BuildConfig, Box<dyn Error>> {
	print_banner();

	let theme = ColorfulTheme {
		values_style: Style::new().yellow().dim(),
		..ColorfulTheme::default()
	};

	let mut config = BuildConfig::default();

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
		let arch_index = Select::with_theme(&theme)
			.with_prompt("Choose image architecture")
			.default(0)
			.item("amd64")
			.item("aarch64")
			.item("i386")
			.interact()?;

		config.arch = match arch_index {
			0 => Architecture::amd64,
			1 => Architecture::arm64,
			2 => Architecture::i386,
			_ => panic!(),
		};
	}

	// Prompt template
	let template_index = Select::with_theme(&theme)
		.with_prompt("Choose image base template")
		.item("Alpine Linux")
		.item("Arch Linux")
		.item("Debian")
		.item("macOS")
		.item("Pop_OS!")
		.item("Steam Deck OS")
		.item("Steam OS")
		.item("Ubuntu")
		.item("Windows 7")
		.item("Windows 10")
		.item("Windows 11")
		.interact()?;

	match template_index {
		0 => AlpineLinuxTemplate::prompt(&config, &theme)?,
		1 => ArchLinuxTemplate::prompt(&config, &theme)?,
		_ => panic!(),
	};

	Ok(config)
}

pub fn run(cmd: crate::cmd::Commands) -> Result<(), Box<dyn Error>> {
	match cmd {
		Commands::Init {
			name,
			template,
			arch,
			memory,
			disk,
			list_templates,
			mimic_hardware,
		} => {
			let config_path = Path::new("goldboot.json");

			if config_path.exists() {
				bail!("This directory has already been initialized. Delete goldboot.json to reinitialize.");
			}

			let config = if template.len() == 0 {
				interactive_config()?
			} else {
				// Create a new config to be filled in according to the given arguments
				let mut config = BuildConfig::default();

				if let Some(name) = name {
					config.name = name.to_string();
				} else {
					// Set name equal to directory name
					if let Some(name) = std::env::current_dir()?.file_name() {
						config.name = name.to_str().unwrap().to_string();
					}
				}

				// Generate QEMU flags for this hardware
				//config.qemuargs = generate_qemuargs()?;

				// Set architecture if given
				if let Some(arch) = arch {
					config.arch = arch.to_owned().try_into()?;
				}

				// Run template-specific initialization
				let mut default_templates = Vec::new();
				for t in template {
					let t: TemplateBase =
						serde_json::from_str(format!("{{\"base\": \"{}\"}}", &t).as_str())?;
					default_templates.push(t.get_default_template()?);
				}
				config.templates = default_templates;
				config
			};

			// Finally write out the config
			std::fs::write(config_path, serde_json::to_string_pretty(&config)?)?;
			Ok(())
		}
		_ => panic!(),
	}
}
