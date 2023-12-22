use anyhow::Result;
use chrono::TimeZone;
use console::Style;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};
use ubyte::ToByteUnit;

pub fn run(cmd: super::Commands) -> Result<()> {
    match cmd {
        super::Commands::Image { command } => match &command {
            super::ImageCommands::List {} => {
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
            super::ImageCommands::Info { image } => {
                if let Some(image) = image {
                    let image = ImageLibrary::find_by_id(image)?;
                    // TODO
                }

                Ok(())
            }
        },
        _ => panic!(),
    }
}
