use std::process::ExitCode;
use tracing::info;

use super::RegistryCommands;
use crate::{
    library::ImageLibrary,
    registry::{Client, parse_image_ref},
};

/// Combine a `--username`/`--password` pair into the `Option<(u, p)>` shape
/// the client expects. Both must be present; either alone is an error.
fn resolve_auth(
    username: Option<String>,
    password: Option<String>,
) -> Result<Option<(String, String)>, &'static str> {
    match (username, password) {
        (Some(u), Some(p)) => Ok(Some((u, p))),
        (None, None) => Ok(None),
        (Some(_), None) => Err("--username given without --password"),
        (None, Some(_)) => Err("--password given without --username"),
    }
}

pub fn run(cmd: super::Commands) -> ExitCode {
    match cmd {
        super::Commands::Registry { command } => match command {
            RegistryCommands::Pull {
                reference,
                username,
                password,
            } => {
                let (host, name, tag) = match parse_image_ref(&reference) {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("Invalid reference: {e}");
                        return ExitCode::FAILURE;
                    }
                };
                let auth = match resolve_auth(username, password) {
                    Ok(a) => a,
                    Err(e) => {
                        eprintln!("{e}");
                        return ExitCode::FAILURE;
                    }
                };

                let client = match Client::new(&host, auth) {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("Bad registry address: {e}");
                        return ExitCode::FAILURE;
                    }
                };

                let library = ImageLibrary::open();
                let tmp = library.temporary();
                info!("Pulling {reference}");
                if let Err(e) = client.pull_to_file(&name, &tag, &tmp) {
                    eprintln!("Pull failed: {e}");
                    let _ = std::fs::remove_file(&tmp);
                    return ExitCode::FAILURE;
                }
                if let Err(e) = library.add_move(&tmp) {
                    eprintln!("Failed to add image to library: {e}");
                    let _ = std::fs::remove_file(&tmp);
                    return ExitCode::FAILURE;
                }
                println!("Pulled {reference}");
                ExitCode::SUCCESS
            }

            RegistryCommands::Push {
                reference,
                username,
                password,
            } => {
                let (host, name, tag) = match parse_image_ref(&reference) {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("Invalid reference: {e}");
                        return ExitCode::FAILURE;
                    }
                };
                let auth = match resolve_auth(username, password) {
                    Ok(a) => a,
                    Err(e) => {
                        eprintln!("{e}");
                        return ExitCode::FAILURE;
                    }
                };

                let image = match ImageLibrary::find_by_name(&name) {
                    Ok(img) => img,
                    Err(e) => {
                        eprintln!("Image '{name}' not found in local library: {e}");
                        return ExitCode::FAILURE;
                    }
                };

                let file_len = match std::fs::metadata(&image.path) {
                    Ok(m) => m.len(),
                    Err(e) => {
                        eprintln!("Failed to stat image: {e}");
                        return ExitCode::FAILURE;
                    }
                };
                let file = match std::fs::File::open(&image.path) {
                    Ok(f) => f,
                    Err(e) => {
                        eprintln!("Failed to open image: {e}");
                        return ExitCode::FAILURE;
                    }
                };

                let client = match Client::new(&host, auth) {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("Bad registry address: {e}");
                        return ExitCode::FAILURE;
                    }
                };

                info!("Pushing {} ({} bytes)", image.path.display(), file_len);
                if let Err(e) = client.push_image(&name, &tag, file, file_len) {
                    eprintln!("Push failed: {e}");
                    return ExitCode::FAILURE;
                }
                println!("Pushed {reference}");
                ExitCode::SUCCESS
            }
        },
        _ => panic!(),
    }
}
