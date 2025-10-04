use std::process::ExitCode;

use crate::library::ImageLibrary;
use chrono::TimeZone;

use ubyte::ToByteUnit;

pub fn run(cmd: super::Commands) -> ExitCode {
    match cmd {
        super::Commands::Image { command } => match &command {
            super::ImageCommands::List {} => {
                let images = ImageLibrary::find_all().unwrap();

                println!(
                    "Image Name      Image Size   Build Date                      Image ID     Description"
                );
                for image in images {
                    println!(
                        "{:15} {:12} {:31} {:12} {}",
                        std::str::from_utf8(&image.primary_header.name).unwrap(),
                        image.primary_header.size.bytes().to_string(),
                        chrono::Utc
                            .timestamp(image.primary_header.timestamp as i64, 0)
                            .to_rfc2822(),
                        &image.id[0..12],
                        "TODO",
                    );
                }
                ExitCode::SUCCESS
            }
            super::ImageCommands::Info { image } => {
                if let Some(image) = image {
                    let _image = ImageLibrary::find_by_id(image).unwrap();
                    // TODO
                }

                ExitCode::SUCCESS
            }
        },
        _ => panic!(),
    }
}
