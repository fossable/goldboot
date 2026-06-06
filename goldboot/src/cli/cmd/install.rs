use crate::library::ImageLibrary;
use console::Style;
use dialoguer::{Confirm, theme::ColorfulTheme};
use std::{io::IsTerminal, path::Path, process::ExitCode};
use tracing::{error, warn};

pub fn run(cmd: super::Commands) -> ExitCode {
    match cmd {
        super::Commands::Install {
            dest,
            include,
            dryrun,
            takeover,
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

            if !dryrun && !is_efi_partition(&dest) {
                error!(dest = %dest, "Destination path is not a mounted EFI partition (must be vfat with EFI System Partition type)");
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

            // Find any explicitly requested images by reference.
            for reference in &include {
                let r = match crate::registry::ImageRef::parse(reference) {
                    Ok(r) => r,
                    Err(err) => {
                        error!(reference = %reference, error = ?err, "Invalid image reference");
                        return ExitCode::FAILURE;
                    }
                };
                match library.find_by_ref(&r) {
                    Ok(handle) => images.push(handle.path),
                    Err(err) => {
                        error!(reference = %reference, error = ?err, "Failed to find image");
                        return ExitCode::FAILURE;
                    }
                }
            }

            let gb_dir = Path::new(&dest).join("goldboot");
            let efi_dest = if takeover {
                #[cfg(target_arch = "aarch64")]
                let name = "BOOTAA64.EFI";
                #[cfg(not(target_arch = "aarch64"))]
                let name = "BOOTX64.EFI";
                Path::new(&dest).join("EFI/BOOT").join(name)
            } else {
                gb_dir.join("goldboot.efi")
            };

            // Build a plan description shared by --dryrun and the TTY confirmation prompt.
            let mut plan_lines: Vec<String> = Vec::new();
            plan_lines.push("Files that would be written:".to_owned());
            plan_lines.push(format!(
                "  {} -> {}  ({})",
                efi_src.display(),
                efi_dest.display(),
                if efi_dest.exists() {
                    "overwrite"
                } else {
                    "new"
                }
            ));
            for image_path in &images {
                let dest_path = gb_dir.join(image_path.file_name().unwrap());
                plan_lines.push(format!(
                    "  {} -> {}  ({})",
                    image_path.display(),
                    dest_path.display(),
                    if dest_path.exists() {
                        "overwrite"
                    } else {
                        "new"
                    }
                ));
            }
            if dest == "/boot" {
                plan_lines.push(String::new());
                plan_lines.push("EFI variables that would be changed:".to_owned());
                match describe_next_boot("goldboot.efi") {
                    Ok(description) => plan_lines.push(description),
                    Err(err) => plan_lines.push(format!(
                        "  (could not determine EFI variable changes: {err})"
                    )),
                }
            }

            if dryrun {
                println!("Dry run — no changes will be made.\n");
                for line in &plan_lines {
                    println!("{line}");
                }
                return ExitCode::SUCCESS;
            }

            if std::io::stdout().is_terminal() {
                for line in &plan_lines {
                    println!("{line}");
                }
                println!();
                if !Confirm::with_theme(&theme)
                    .with_prompt("Proceed with installation?")
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
            if takeover {
                let efi_boot_dir = Path::new(&dest).join("EFI/BOOT");
                if let Err(err) = std::fs::create_dir_all(&efi_boot_dir) {
                    error!(error = ?err, "Failed to create EFI/BOOT directory");
                    return ExitCode::FAILURE;
                }
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

            // When installing to /boot (non-takeover), set BootNext so the firmware boots
            // goldboot.efi on the next reboot. Takeover mode doesn't need this because
            // EFI/BOOT/BOOTX64.EFI is already the firmware's default fallback path.
            if !takeover && dest == "/boot" {
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

/// Return true if `path` is a mount point whose filesystem type is `vfat`.
///
/// Uses [`sysinfo::Disks`] to enumerate active mounts and checks that the
/// entry whose mount point matches `path` reports a `vfat` filesystem — the
/// required type for an EFI System Partition.
fn is_efi_partition(path: &str) -> bool {
    use sysinfo::Disks;

    let target = Path::new(path);
    Disks::new_with_refreshed_list()
        .list()
        .iter()
        .any(|d| d.mount_point() == target && d.file_system().eq_ignore_ascii_case("vfat"))
}
