use dialoguer::{Input, Password, theme::ColorfulTheme};
use std::process::ExitCode;
use tracing::info;
use zeroize::Zeroize;

use super::RegistryCommands;
use crate::{
    library::ImageLibrary,
    registry::{Client, RegistryCredentials, RegistryEntry, parse_image_ref},
};

pub fn run(cmd: super::Commands) -> ExitCode {
    let theme = ColorfulTheme::default();

    match cmd {
        super::Commands::Registry { command } => match command {
            RegistryCommands::Login { registry } => {
                let username: String = match Input::with_theme(&theme)
                    .with_prompt(format!("Username for {registry}"))
                    .interact_text()
                {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("Input cancelled: {e}");
                        return ExitCode::FAILURE;
                    }
                };
                let mut password: String = match Password::with_theme(&theme)
                    .with_prompt("Password")
                    .interact()
                {
                    Ok(p) => p,
                    Err(e) => {
                        eprintln!("Input cancelled: {e}");
                        return ExitCode::FAILURE;
                    }
                };

                let mut client = match Client::new(&registry) {
                    Ok(c) => c,
                    Err(e) => {
                        password.zeroize();
                        eprintln!("Bad registry address: {e}");
                        return ExitCode::FAILURE;
                    }
                };
                let login_result = client.login(&username, &password);
                password.zeroize();
                let perms = match login_result {
                    Ok(p) => p,
                    Err(e) => {
                        eprintln!("Login failed: {e}");
                        return ExitCode::FAILURE;
                    }
                };

                let token = client.token().expect("token after login").to_string();
                let mut creds = RegistryCredentials::load().unwrap_or_default();
                creds
                    .registries
                    .insert(registry.clone(), RegistryEntry { token });
                if let Err(e) = creds.save() {
                    eprintln!("Failed to save credentials: {e}");
                    return ExitCode::FAILURE;
                }
                println!(
                    "Logged in to {registry} as {username} (pull={} push={})",
                    perms.pull, perms.push
                );
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

                let mut client = match Client::new(&host) {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("Bad registry address: {e}");
                        return ExitCode::FAILURE;
                    }
                };
                client.set_token_from_storage(token);

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

                let mut client = match Client::new(&host) {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("Bad registry address: {e}");
                        return ExitCode::FAILURE;
                    }
                };
                client.set_token_from_storage(token);

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
