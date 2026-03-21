use crate::library::ImageLibrary;
use console::Style;
use dialoguer::{Confirm, theme::ColorfulTheme};
use std::{path::Path, process::ExitCode};
use tracing::error;

pub fn run(cmd: super::Commands) -> ExitCode {
    match cmd {
        super::Commands::Install { dest, include } => {
            let theme = ColorfulTheme {
                values_style: Style::new().yellow().dim(),
                ..ColorfulTheme::default()
            };

            // If we're not root, ask for confirmation and re-invoke with sudo
            if !whoami::username().map(|u| u == "root").unwrap_or(false) {
                if !Confirm::with_theme(&theme)
                    .with_prompt("Root privileges are required. Re-invoke with sudo?")
                    .interact()
                    .unwrap()
                {
                    return ExitCode::FAILURE;
                }

                let args: Vec<String> = std::env::args().collect();
                let status = std::process::Command::new("sudo").args(&args).status();

                return match status {
                    Ok(s) if s.success() => ExitCode::SUCCESS,
                    _ => ExitCode::FAILURE,
                };
            }

            if !Path::new(&dest).exists() {
                error!(dest = %dest, "Destination path does not exist");
                return ExitCode::FAILURE;
            }

            // Gather all requested images
            let mut images = Vec::new();

            // Find goldboot.efi in the lib directory
            let library = ImageLibrary::open();
            let efi_src = library.directory.parent().unwrap().join("goldboot.efi");
            if !efi_src.exists() {
                error!(path = %efi_src.display(), "goldboot.efi not found in lib directory");
                return ExitCode::FAILURE;
            }

            // Find any explicitly requested images
            for id in &include {
                match ImageLibrary::find_by_id(id) {
                    Ok(handle) => images.push(handle.path),
                    Err(err) => {
                        error!(id = %id, error = ?err, "Failed to find image");
                        return ExitCode::FAILURE;
                    }
                }
            }

            // Determine the EFI boot filename based on current architecture
            let efi_filename = if cfg!(target_arch = "x86_64") {
                "BootX64.efi"
            } else if cfg!(target_arch = "aarch64") {
                "BootAA64.efi"
            } else {
                error!("Unsupported architecture");
                return ExitCode::FAILURE;
            };

            let efi_dir = Path::new(&dest).join("EFI").join("Boot");
            let efi_dest = efi_dir.join(efi_filename);
            let gb_dir = Path::new(&dest).join("goldboot");

            // Collect and log files that will be overwritten before prompting
            let mut overwrites: Vec<std::path::PathBuf> = Vec::new();
            if efi_dest.exists() {
                overwrites.push(efi_dest.clone());
            }
            for p in &images {
                let dest_path = gb_dir.join(p.file_name().unwrap());
                if dest_path.exists() {
                    overwrites.push(dest_path);
                }
            }

            if !overwrites.is_empty() {
                for path in &overwrites {
                    println!("Will overwrite: {}", path.display());
                }
                if !Confirm::with_theme(&theme)
                    .with_prompt(format!("Do you want to overwrite files in: {}?", dest))
                    .interact()
                    .unwrap()
                {
                    return ExitCode::FAILURE;
                }
            }

            // Create directories
            if let Err(err) = std::fs::create_dir_all(&efi_dir) {
                error!(error = ?err, "Failed to create EFI directory");
                return ExitCode::FAILURE;
            }
            if let Err(err) = std::fs::create_dir_all(&gb_dir) {
                error!(error = ?err, "Failed to create goldboot directory");
                return ExitCode::FAILURE;
            }

            // Copy goldboot.efi to the EFI boot location
            if let Err(err) = std::fs::copy(&efi_src, &efi_dest) {
                error!(error = ?err, dest = %efi_dest.display(), "Failed to copy EFI binary");
                return ExitCode::FAILURE;
            }

            // Copy .gb image files to goldboot directory
            for image_path in &images {
                let dest_path = gb_dir.join(image_path.file_name().unwrap());
                if let Err(err) = std::fs::copy(image_path, &dest_path) {
                    error!(error = ?err, dest = %dest_path.display(), "Failed to copy image");
                    return ExitCode::FAILURE;
                }
            }

            ExitCode::SUCCESS
        }
        _ => panic!(),
    }
}
