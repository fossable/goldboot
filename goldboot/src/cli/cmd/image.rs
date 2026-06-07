use std::process::ExitCode;

use crate::{
    library::ImageLibrary,
    registry::{Client, ImageRef, host_without_scheme},
};
use ubyte::ToByteUnit;

#[allow(unreachable_code)]
pub fn run(cmd: super::Commands) -> ExitCode {
    match cmd {
        super::Commands::Image { command } => match command {
            super::ImageCommands::List {
                registry,
                username,
                password,
            } => list(registry, username, password),
            super::ImageCommands::Info { image } => info(image),
            super::ImageCommands::Delete { images } => delete(images),
            super::ImageCommands::Push {
                reference,
                username,
                password,
            } => super::registry::push(reference, username, password),
            super::ImageCommands::Pull {
                reference,
                username,
                password,
            } => super::registry::pull(reference, username, password),
        },
        _ => panic!(),
    }
}

fn render_ref(host: Option<&str>, name: &str, tag: &str) -> String {
    let mut r = ImageRef::new(name).with_tag(tag);
    if let Some(h) = host {
        r = r.with_host(h);
    }
    r.to_string()
}

fn list(registry: Option<String>, username: Option<String>, password: Option<String>) -> ExitCode {
    match registry {
        None => {
            if username.is_some() || password.is_some() {
                eprintln!("--username/--password are only valid when a registry is specified");
                return ExitCode::FAILURE;
            }
            let library = ImageLibrary::open();
            let images = match library.find_all() {
                Ok(i) => i,
                Err(e) => {
                    eprintln!("Failed to read image library: {e}");
                    return ExitCode::FAILURE;
                }
            };

            println!(
                "{:50} {:12} {:12} {:31} {:12}",
                "Image", "Minimum Size", "Disk Size", "Build Date", "Content ID"
            );
            for (host, image) in images {
                println!(
                    "{:50} {:12} {:12} {:31} {}",
                    render_ref(
                        host.as_deref(),
                        &image.primary_header.name_str(),
                        &image.primary_header.tag_str(),
                    ),
                    image.primary_header.size.bytes().to_string(),
                    image.file_size.bytes().to_string(),
                    chrono::DateTime::from_timestamp(image.primary_header.timestamp as i64, 0)
                        .unwrap_or_default()
                        .to_rfc2822(),
                    &image.id[..image.id.len().min(12)],
                );
            }
            ExitCode::SUCCESS
        }
        Some(address) => {
            let auth = match super::registry::resolve_auth(username, password) {
                Ok(a) => a,
                Err(e) => {
                    eprintln!("{e}");
                    return ExitCode::FAILURE;
                }
            };
            let client = match Client::new(&address, auth) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Bad registry address: {e}");
                    return ExitCode::FAILURE;
                }
            };
            let images = match client.list_images() {
                Ok(i) => i,
                Err(e) => {
                    eprintln!("Failed to list registry images: {e}");
                    return ExitCode::FAILURE;
                }
            };

            let host_display = host_without_scheme(&address);

            println!(
                "{:50} {:12} {:12} {:31} {:12}",
                "Image", "Minimum Size", "Disk Size", "Build Date", "Content ID"
            );
            for entry in images {
                println!(
                    "{:50} {:12} {:12} {:31} {}",
                    render_ref(Some(host_display), &entry.name, &entry.tag),
                    entry.size.bytes().to_string(),
                    entry.file_size.bytes().to_string(),
                    chrono::DateTime::from_timestamp(entry.timestamp as i64, 0)
                        .unwrap_or_default()
                        .to_rfc2822(),
                    &entry.id[..entry.id.len().min(12)],
                );
            }
            ExitCode::SUCCESS
        }
    }
}

fn info(reference: String) -> ExitCode {
    let r = match ImageRef::parse(&reference) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Invalid reference '{reference}': {e}");
            return ExitCode::FAILURE;
        }
    };

    let library = ImageLibrary::open();
    let image = match library.find_by_ref(&r) {
        Ok(img) => img,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };

    println!(
        "Reference:   {}",
        render_ref(
            r.host_bare(),
            &image.primary_header.name_str(),
            &image.primary_header.tag_str(),
        )
    );
    println!("Content ID:  {}", image.id);
    println!("Size:        {}", image.primary_header.size.bytes());
    println!(
        "Date:        {}",
        chrono::DateTime::from_timestamp(image.primary_header.timestamp as i64, 0)
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

fn delete(references: Vec<String>) -> ExitCode {
    let library = ImageLibrary::open();
    let mut failed = false;
    for reference in &references {
        let mut r = match ImageRef::parse(reference) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Invalid reference '{reference}': {e}");
                failed = true;
                continue;
            }
        };

        // Resolve `None` tag to the newest matching tag — delete demands
        // a concrete file.
        if r.tag.is_none() {
            match library.find_by_ref(&r) {
                Ok(h) => r.tag = Some(h.primary_header.tag_str()),
                Err(e) => {
                    eprintln!("Failed to delete {reference}: {e}");
                    failed = true;
                    continue;
                }
            }
        }

        if let Err(e) = library.delete(&r) {
            eprintln!("Failed to delete {reference}: {e}");
            failed = true;
        }
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
