use crate::{
    boot::{EspInfo, describe_boot_entry, register_boot_entry},
    library::ImageLibrary,
};
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
                let description = match esp_from_boot_mount() {
                    Ok(esp) => describe_boot_entry(&esp, "goldboot", "\\EFI\\Boot\\goldboot.efi")
                        .unwrap_or_else(|err| {
                            format!("  (could not determine EFI variable changes: {err})")
                        }),
                    Err(err) => {
                        format!("  (could not determine EFI variable changes: {err})")
                    }
                };
                plan_lines.push(description);
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
                match esp_from_boot_mount() {
                    Ok(esp) => {
                        if let Err(err) =
                            register_boot_entry(&esp, "goldboot", "\\EFI\\Boot\\goldboot.efi")
                        {
                            warn!(error = ?err, "Failed to set BootNext EFI variable; boot entry must be created manually");
                        }
                    }
                    Err(err) => {
                        warn!(error = ?err, "Failed to discover ESP from /boot mount; BootNext not set");
                    }
                }
            }

            ExitCode::SUCCESS
        }
        _ => panic!(),
    }
}

/// Build an [`EspInfo`] for the partition currently mounted at `/boot`. Used
/// by the `install` command, which writes goldboot.efi onto the running
/// system's ESP rather than a freshly-deployed disk.
fn esp_from_boot_mount() -> anyhow::Result<EspInfo> {
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

    let (partuuid_str, part_start_lba, part_size_bytes) = parse_lsblk(&source)?;
    let partition_guid = partuuid_str.parse::<uuid::Uuid>()?;
    let partition_number: u32 = source
        .trim_start_matches(|c: char| !c.is_ascii_digit())
        .parse()
        .unwrap_or(1);

    Ok(EspInfo {
        partition_number,
        partition_start_lba: part_start_lba,
        // lsblk SIZE is in bytes (because --bytes); EFI HARD_DRIVE wants LBAs.
        partition_size_lba: part_size_bytes / 512,
        partition_guid,
    })
}

/// Run `lsblk --pairs` on `device` and return `(partuuid, start_lba, size_bytes)`.
///
/// `lsblk START` is always reported in 512-byte sectors; `SIZE` is in bytes
/// because we pass `--bytes`. Callers must convert size to LBAs themselves if
/// they need consistent units.
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
