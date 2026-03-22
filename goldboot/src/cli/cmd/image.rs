use std::process::ExitCode;

use crate::library::ImageLibrary;
use ubyte::ToByteUnit;

#[allow(unreachable_code)]
pub fn run(cmd: super::Commands) -> ExitCode {
    match cmd {
        super::Commands::Image { command } => match &command {
            super::ImageCommands::List {} => {
                let images = ImageLibrary::find_all().unwrap();

                println!(
                    "{:40} {:12} {:31} {:12}",
                    "Image Elements", "Image Size", "Build Date", "Image ID"
                );
                for image in images {
                    let elements: Vec<String> = image
                        .primary_header
                        .elements
                        .iter()
                        .map(|e| format!("{}:{}", e.os(), e.name()))
                        .collect();
                    println!(
                        "{:40} {:12} {:31} {}",
                        elements.join(", "),
                        image.primary_header.size.bytes().to_string(),
                        chrono::DateTime::from_timestamp(
                            image.primary_header.timestamp as i64,
                            0
                        )
                        .unwrap_or_default()
                        .to_rfc2822(),
                        &image.id[0..12],
                    );
                }
                ExitCode::SUCCESS
            }
            super::ImageCommands::Info { image } => {
                let id = match image {
                    Some(id) => id.clone(),
                    None => {
                        eprintln!("No image ID specified");
                        return ExitCode::FAILURE;
                    }
                };

                let image = match ImageLibrary::find_by_id(&id) {
                    Ok(img) => img,
                    Err(e) => {
                        eprintln!("Image not found: {e}");
                        return ExitCode::FAILURE;
                    }
                };

                println!("ID:          {}", image.id);
                println!("Size:        {}", image.primary_header.size.bytes());
                println!(
                    "Date:        {}",
                    chrono::DateTime::from_timestamp(
                        image.primary_header.timestamp as i64,
                        0
                    )
                    .unwrap_or_default()
                    .to_rfc2822()
                );
                println!("Arch:        {:?}", image.primary_header.arch);
                println!("Encryption:  {:?}", image.primary_header.encryption_type);
                println!("Elements:    {}", image.primary_header.element_count);
                for (i, element) in image.primary_header.elements.iter().enumerate() {
                    println!("  [{}] os={} name={}", i, element.os(), element.name());
                }

                ExitCode::SUCCESS
            }
            super::ImageCommands::Delete { images } => {
                let library = ImageLibrary::open();
                let mut failed = false;
                for image in images {
                    if let Err(e) = library.delete(image) {
                        eprintln!("Failed to delete {image}: {e}");
                        failed = true;
                    }
                }
                if failed { ExitCode::FAILURE } else { ExitCode::SUCCESS }
            }
        },
        _ => panic!(),
    }
}
