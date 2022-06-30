use console::Style;
use crate::cmd::Commands;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};

pub fn run(cmd: crate::cmd::Commands) -> Result<(), Box<dyn Error>> {

    match cmd {
        Commands::Write {
			image,
			output,
			confirm,
		} => {
			let image = ImageLibrary::find_by_id(image)?;

			if Path::new(output).exists() && !*confirm {
				// Prompt to continue
				print!("Confirm? [Y/N]");
				let mut answer = String::new();
				std::io::stdin().read_line(&mut answer)?;

				match answer.as_str() {
					"y" => {}
					"Y" => {}
					_ => std::process::exit(0),
				}
			}

			image.write(output)
		},
        _ => panic!(),
    }
}
