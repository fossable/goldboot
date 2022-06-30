use console::Style;
use crate::cmd::Commands;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};

pub fn run(cmd: crate::cmd::Commands) -> Result<(), Box<dyn Error>> {

    match cmd {
        Commands::Image { command } => match &command {
			ImageCommands::List {} => {
				let images = ImageLibrary::load()?;

				println!("Image Name      Image Size   Build Date                      Image ID     Description");
				for image in images {
					println!(
						"{:15} {:12} {:31} {:12} {}",
						std::str::from_utf8(&image.primary_header.name)?,
						image.primary_header.size.bytes().to_string(),
						chrono::Utc
							.timestamp(image.primary_header.timestamp as i64, 0)
							.to_rfc2822(),
						&image.id[0..12],
						"TODO",
					);
				}
				Ok(())
			}
			ImageCommands::Info { image } => {
				let image = ImageLibrary::find_by_id(image)?;
				// TODO
				Ok(())
			}
			ImageCommands::Run { image } => Ok(()),
		},
        _ => panic!(),
    }
}
