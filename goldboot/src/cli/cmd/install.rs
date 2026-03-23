use crate::library::ImageLibrary;
use console::Style;
use dialoguer::{Confirm, theme::ColorfulTheme};
use std::{path::Path, process::ExitCode};
use tracing::{error, warn};

pub fn run(cmd: super::Commands) -> ExitCode {
    match cmd {
        super::Commands::Install {
            dest,
            include,
            dryrun,
        } => {
            let theme = ColorfulTheme {
                values_style: Style::new().yellow().dim(),
                ..ColorfulTheme::default()
            };

            // If we're not root, ask for confirmation and re-invoke with sudo
            if !dryrun && !whoami::username().map(|u| u == "root").unwrap_or(false) {
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

            if !dryrun && !Path::new(&dest).exists() {
                error!(dest = %dest, "Destination path does not exist");
                return ExitCode::FAILURE;
            }

            // Gather all requested images
            let mut images = Vec::new();

            // Find goldboot.efi in the lib directory
            let library = ImageLibrary::open();
            let efi_src = library.directory.parent().unwrap().join("goldboot.efi");
            if !dryrun && !efi_src.exists() {
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

            let gb_dir = Path::new(&dest).join("goldboot");
            let efi_dest = gb_dir.join("goldboot.efi");

            if dryrun {
                println!("Dry run — no changes will be made.\n");
                println!("Files that would be written:");
                println!(
                    "  {} -> {}  ({})",
                    efi_src.display(),
                    efi_dest.display(),
                    if efi_dest.exists() {
                        "overwrite"
                    } else {
                        "new"
                    }
                );
                for image_path in &images {
                    let dest_path = gb_dir.join(image_path.file_name().unwrap());
                    println!(
                        "  {} -> {}  ({})",
                        image_path.display(),
                        dest_path.display(),
                        if dest_path.exists() {
                            "overwrite"
                        } else {
                            "new"
                        }
                    );
                }

                if dest == "/boot" {
                    println!("\nEFI variables that would be changed:");
                    match describe_next_boot("goldboot.efi") {
                        Ok(description) => println!("{description}"),
                        Err(err) => {
                            println!("  (could not determine EFI variable changes: {err})");
                        }
                    }
                }

                return ExitCode::SUCCESS;
            }

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

            // When installing to /boot, set BootNext so the firmware boots goldboot.efi on the
            // next reboot.
            if dest == "/boot" {
                if let Err(err) = set_next_boot(&efi_dest.to_string_lossy(), "goldboot.efi") {
                    warn!(error = ?err, "Failed to set BootNext EFI variable; boot entry must be created manually");
                }
            }

            ExitCode::SUCCESS
        }
        _ => panic!(),
    }
}

/// Describe what EFI variable changes `set_next_boot` would make, without writing anything.
fn describe_next_boot(efi_filename: &str) -> anyhow::Result<String> {
    use efivar::{
        boot::{EFIHardDrive, EFIHardDriveType, FilePath, FilePathList},
        efi::Variable,
    };

    let source = {
        let out = std::process::Command::new("findmnt")
            .args(["--noheadings", "--output", "SOURCE", "/boot"])
            .output()?;
        let s = String::from_utf8(out.stdout)?.trim().to_owned();
        if s.is_empty() {
            anyhow::bail!("/boot is not a separate mount point");
        }
        s
    };

    let (partuuid_str, part_start, part_size) = parse_lsblk(&source)?;
    let partition_sig = partuuid_str.parse::<uuid::Uuid>()?;
    let partition_number: u32 = source
        .trim_start_matches(|c: char| !c.is_ascii_digit())
        .parse()
        .unwrap_or(1);

    let hard_drive = EFIHardDrive {
        partition_number,
        partition_start: part_start,
        partition_size: part_size,
        partition_sig,
        format: 0x02,
        sig_type: EFIHardDriveType::Gpt,
    };
    let file_path = FilePath {
        path: format!("\\EFI\\Boot\\{efi_filename}"),
    };
    let file_path_list = FilePathList {
        file_path,
        hard_drive,
    };

    let manager = efivar::system();

    // Determine the boot entry ID that would be used
    let existing_id = manager.get_boot_order().ok().and_then(|order| {
        order.into_iter().find(|&id| {
            let var = Variable::new(&format!("Boot{:04X}", id));
            manager
                .read(&var)
                .ok()
                .and_then(|(data, _)| efivar::boot::BootEntry::parse(data).ok())
                .map(|e| e.description == "goldboot")
                .unwrap_or(false)
        })
    });

    let boot_id = if let Some(id) = existing_id {
        id
    } else {
        let used: std::collections::HashSet<u16> = manager
            .get_boot_order()
            .unwrap_or_default()
            .into_iter()
            .collect();
        (0x0100u16..=0xFFFF)
            .find(|id| !used.contains(id))
            .ok_or_else(|| anyhow::anyhow!("No free boot entry slots"))?
    };

    let action = if existing_id.is_some() {
        "overwrite existing"
    } else {
        "create new"
    };

    Ok(format!(
        "  Boot{:04X}  ({action})  goldboot  {file_path_list}\n  BootNext = Boot{:04X}",
        boot_id, boot_id,
    ))
}

/// Find or create a boot entry for goldboot.efi and set `BootNext` to it.
///
/// `efi_dest` is the full path to the installed EFI binary (used only for logging).
/// `efi_filename` is the short filename used in the EFI path (e.g. "BootX64.efi").
fn set_next_boot(efi_dest: &str, efi_filename: &str) -> anyhow::Result<()> {
    use efivar::{
        boot::{
            BootEntry, BootEntryAttributes, EFIHardDrive, EFIHardDriveType, FilePath, FilePathList,
        },
        efi::{Variable, VariableFlags},
    };

    let source = {
        let out = std::process::Command::new("findmnt")
            .args(["--noheadings", "--output", "SOURCE", "/boot"])
            .output()?;
        let s = String::from_utf8(out.stdout)?.trim().to_owned();
        if s.is_empty() {
            anyhow::bail!("/boot does not appear to be a separate mount point");
        }
        s
    };

    let (partuuid_str, part_start, part_size) = parse_lsblk(&source)?;
    let partition_sig = partuuid_str.parse::<uuid::Uuid>()?;
    let partition_number: u32 = source
        .trim_start_matches(|c: char| !c.is_ascii_digit())
        .parse()
        .unwrap_or(1);

    let hard_drive = EFIHardDrive {
        partition_number,
        partition_start: part_start,
        partition_size: part_size,
        partition_sig,
        format: 0x02, // GPT
        sig_type: EFIHardDriveType::Gpt,
    };

    let file_path = FilePath {
        path: format!("\\EFI\\Boot\\{efi_filename}"),
    };
    let file_path_list = FilePathList {
        file_path,
        hard_drive,
    };

    let entry = BootEntry {
        attributes: BootEntryAttributes::LOAD_OPTION_ACTIVE,
        description: "goldboot".to_owned(),
        file_path_list: Some(file_path_list),
        optional_data: vec![],
    };

    let mut manager = efivar::system();

    // Look for an existing goldboot boot entry to reuse its ID
    let boot_id = if let Ok(order) = manager.get_boot_order() {
        order.into_iter().find(|&id| {
            let var = Variable::new(&format!("Boot{:04X}", id));
            manager
                .read(&var)
                .ok()
                .and_then(|(data, _)| efivar::boot::BootEntry::parse(data).ok())
                .map(|e| e.description == "goldboot")
                .unwrap_or(false)
        })
    } else {
        None
    };

    let boot_id = if let Some(id) = boot_id {
        // Overwrite the existing entry
        manager.add_boot_entry(id, entry)?;
        id
    } else {
        // Find a free boot entry slot (start at 0x0100 to avoid conflicts)
        let used: std::collections::HashSet<u16> = manager
            .get_boot_order()
            .unwrap_or_default()
            .into_iter()
            .collect();
        let new_id = (0x0100u16..=0xFFFF)
            .find(|id| !used.contains(id))
            .ok_or_else(|| anyhow::anyhow!("No free boot entry slots"))?;
        manager.add_boot_entry(new_id, entry)?;
        new_id
    };

    // Write BootNext as a little-endian u16
    let boot_next_bytes = boot_id.to_le_bytes();
    manager.write(
        &Variable::new("BootNext"),
        VariableFlags::NON_VOLATILE
            | VariableFlags::BOOTSERVICE_ACCESS
            | VariableFlags::RUNTIME_ACCESS,
        &boot_next_bytes,
    )?;

    tracing::info!(
        boot_id = format!("Boot{:04X}", boot_id),
        path = efi_dest,
        "Set BootNext to goldboot.efi"
    );

    Ok(())
}

/// Run `lsblk --pairs` on `device` and return `(partuuid, start_bytes, size_bytes)`.
fn parse_lsblk(device: &str) -> anyhow::Result<(String, u64, u64)> {
    let out = std::process::Command::new("lsblk")
        .args([
            "--noheadings",
            "--output",
            "PARTUUID,START,SIZE",
            "--bytes",
            "--pairs",
            device,
        ])
        .output()?;
    let text = String::from_utf8(out.stdout)?;

    let mut partuuid = String::new();
    let mut start: u64 = 0;
    let mut size: u64 = 0;

    for token in text.split_whitespace() {
        if let Some(v) = token.strip_prefix("PARTUUID=") {
            partuuid = v.trim_matches('"').to_owned();
        } else if let Some(v) = token.strip_prefix("START=") {
            start = v.trim_matches('"').parse().unwrap_or(0);
        } else if let Some(v) = token.strip_prefix("SIZE=") {
            size = v.trim_matches('"').parse().unwrap_or(0);
        }
    }

    if partuuid.is_empty() {
        anyhow::bail!("Could not determine PARTUUID for {device}");
    }

    Ok((partuuid, start, size))
}
