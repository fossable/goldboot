use dialoguer::{Password, theme::ColorfulTheme};
use std::process::ExitCode;
use tracing::info;

use super::RegistryCommands;
use crate::{
    library::ImageLibrary,
    registry::{RegistryCredentials, RegistryEntry, parse_image_ref},
};

pub fn run(cmd: super::Commands) -> ExitCode {
    let theme = ColorfulTheme::default();

    match cmd {
        super::Commands::Registry { command } => match command {
            RegistryCommands::Login { registry } => {
                let token: String = Password::with_theme(&theme)
                    .with_prompt(format!("Enter token for {registry}"))
                    .interact()
                    .unwrap();

                let mut creds = RegistryCredentials::load().unwrap_or_default();
                creds
                    .registries
                    .insert(registry.clone(), RegistryEntry { token });
                if let Err(e) = creds.save() {
                    eprintln!("Failed to save credentials: {e}");
                    return ExitCode::FAILURE;
                }
                println!("Logged in to {registry}");
                ExitCode::SUCCESS
            }

            RegistryCommands::Pull { reference } => {
                let (host, name, tag) = match parse_image_ref(&reference) {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("Invalid reference: {e}");
                        return ExitCode::FAILURE;
                    }
                };

                let creds = match RegistryCredentials::load() {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("Failed to load credentials: {e}");
                        return ExitCode::FAILURE;
                    }
                };
                let token = match creds.token_for(&host) {
                    Ok(t) => t.to_string(),
                    Err(e) => {
                        eprintln!("{e}");
                        return ExitCode::FAILURE;
                    }
                };

                let url = format!("https://{host}/images/{name}/tags/{tag}");
                info!("Pulling {reference} from {url}");

                let library = ImageLibrary::open();
                let tmp = library.temporary();

                let client = reqwest::blocking::Client::new();
                let mut response = match client
                    .get(&url)
                    .header("Authorization", format!("Bearer {token}"))
                    .send()
                {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("Request failed: {e}");
                        return ExitCode::FAILURE;
                    }
                };

                if !response.status().is_success() {
                    eprintln!("Pull failed: HTTP {}", response.status());
                    return ExitCode::FAILURE;
                }

                let mut file = match std::fs::File::create(&tmp) {
                    Ok(f) => f,
                    Err(e) => {
                        eprintln!("Failed to create temp file: {e}");
                        return ExitCode::FAILURE;
                    }
                };

                use crate::cli::progress::ProgressBar;
                let copy_result = if let Some(length) = response.content_length() {
                    ProgressBar::Download.copy(&mut response, &mut file, length)
                } else {
                    std::io::copy(&mut response, &mut file)
                        .map(|_| ())
                        .map_err(anyhow::Error::from)
                };
                if let Err(e) = copy_result {
                    eprintln!("Download failed: {e}");
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

            RegistryCommands::Push { reference } => {
                let (host, name, tag) = match parse_image_ref(&reference) {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("Invalid reference: {e}");
                        return ExitCode::FAILURE;
                    }
                };

                let creds = match RegistryCredentials::load() {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("Failed to load credentials: {e}");
                        return ExitCode::FAILURE;
                    }
                };
                let token = match creds.token_for(&host) {
                    Ok(t) => t.to_string(),
                    Err(e) => {
                        eprintln!("{e}");
                        return ExitCode::FAILURE;
                    }
                };

                // Find a local image whose name matches the reference name
                let image = match ImageLibrary::find_by_name(&name) {
                    Ok(img) => img,
                    Err(e) => {
                        eprintln!("Image '{}' not found in local library: {e}", name);
                        return ExitCode::FAILURE;
                    }
                };

                let url = format!("https://{host}/images/{name}/tags/{tag}");
                info!("Pushing {} to {url}", image.path.display());

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

                let client = reqwest::blocking::Client::new();
                let response = match client
                    .put(&url)
                    .header("Authorization", format!("Bearer {token}"))
                    .header("Content-Length", file_len)
                    .body(reqwest::blocking::Body::sized(file, file_len))
                    .send()
                {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("Request failed: {e}");
                        return ExitCode::FAILURE;
                    }
                };

                if !response.status().is_success() {
                    eprintln!("Push failed: HTTP {}", response.status());
                    return ExitCode::FAILURE;
                }

                println!("Pushed {reference}");
                ExitCode::SUCCESS
            }
        },
        _ => panic!(),
    }
}
