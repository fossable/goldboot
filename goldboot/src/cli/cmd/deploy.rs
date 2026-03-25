use byte_unit::{Byte, UnitType};
use console::Style;
use dialoguer::{Confirm, theme::ColorfulTheme};
use goldboot_image::ImageHandle;
use std::{fs::OpenOptions, io::IsTerminal, path::Path, process::ExitCode};
use tracing::error;

use crate::{
    can_preload, cli::progress::ProgressBar, gpt::fixup_backup_gpt, library::ImageLibrary,
};

pub fn run(cmd: super::Commands) -> ExitCode {
    match cmd {
        super::Commands::Deploy {
            image,
            output,
            confirm,
        } => {
            let theme = ColorfulTheme {
                values_style: Style::new().yellow().dim(),
                ..ColorfulTheme::default()
            };

            let mut image_handle = if Path::new(&image).exists() {
                match ImageHandle::open(&image) {
                    Ok(image_handle) => image_handle,
                    Err(_) => return ExitCode::FAILURE,
                }
            } else {
                match ImageLibrary::find_by_id(&image) {
                    Ok(image_handle) => image_handle,
                    Err(_) => return ExitCode::FAILURE,
                }
            };
            if image_handle.load(None).is_err() {
                return ExitCode::FAILURE;
            }

            let output_path = Path::new(&output);

            // Refuse to deploy to a mounted block device or any of its partitions
            if std::fs::metadata(output_path)
                .map(|m| {
                    use std::os::unix::fs::FileTypeExt;
                    m.file_type().is_block_device()
                })
                .unwrap_or(false)
            {
                let dev = output_path.to_string_lossy();
                let mounts = std::fs::read_to_string("/proc/mounts").unwrap_or_default();
                let mounted = mounts.lines().any(|line| {
                    line.split_whitespace()
                        .next()
                        .map(|d| d == dev || d.starts_with(dev.as_ref()))
                        .unwrap_or(false)
                });
                if mounted {
                    error!(
                        "Refusing to deploy: '{}' or one of its partitions is currently mounted",
                        dev
                    );
                    return ExitCode::FAILURE;
                }
            }

            if output_path.exists() && !confirm {
                if std::io::stderr().is_terminal() {
                    // Print everything we know about the target before asking.
                    let bold = Style::new().bold();
                    let dim = Style::new().dim();
                    let warn = Style::new().yellow().bold();

                    eprintln!();
                    eprintln!("  {} {}", warn.apply_to("TARGET:"), bold.apply_to(&output));

                    // Try block device info for actual block devices, fall back to file metadata.
                    let is_block_device = std::fs::metadata(output_path)
                        .map(|m| {
                            use std::os::unix::fs::FileTypeExt;
                            m.file_type().is_block_device()
                        })
                        .unwrap_or(false);
                    if is_block_device {
                        if let Ok(dev) = block_utils::get_device_info(output_path) {
                            let capacity = Byte::from_u64(dev.capacity)
                                .get_appropriate_unit(UnitType::Binary)
                                .to_string();
                            eprintln!(
                                "  {} {}",
                                dim.apply_to("type:    "),
                                format!("{:?}", dev.device_type)
                            );
                            eprintln!(
                                "  {} {}",
                                dim.apply_to("media:   "),
                                format!("{:?}", dev.media_type)
                            );
                            eprintln!(
                                "  {} {}",
                                dim.apply_to("fs:      "),
                                format!("{:?}", dev.fs_type)
                            );
                            eprintln!("  {} {}", dim.apply_to("capacity:"), capacity);
                            if let Some(serial) = &dev.serial_number {
                                eprintln!("  {} {}", dim.apply_to("serial:  "), serial);
                            }
                            if let Some(lbs) = dev.logical_block_size {
                                eprintln!("  {} {} B", dim.apply_to("lbs:     "), lbs);
                            }
                        }
                    } else if let Ok(meta) = std::fs::metadata(output_path) {
                        let size = Byte::from_u64(meta.len())
                            .get_appropriate_unit(UnitType::Binary)
                            .to_string();
                        eprintln!(
                            "  {} {}",
                            dim.apply_to("type:    "),
                            if meta.is_file() { "file" } else { "other" }
                        );
                        eprintln!("  {} {}", dim.apply_to("size:    "), size);
                        if let Ok(modified) = meta.modified() {
                            if let Ok(elapsed) = modified.elapsed() {
                                let secs = elapsed.as_secs();
                                let age = if secs < 60 {
                                    format!("{}s ago", secs)
                                } else if secs < 3600 {
                                    format!("{}m ago", secs / 60)
                                } else if secs < 86400 {
                                    format!("{}h ago", secs / 3600)
                                } else {
                                    format!("{}d ago", secs / 86400)
                                };
                                eprintln!("  {} {}", dim.apply_to("modified:"), age);
                            }
                        }
                    }

                    // Image write size for comparison.
                    if let Some(size) = image_handle
                        .protected_header
                        .as_ref()
                        .map(|_| image_handle.primary_header.size)
                    {
                        let image_size = Byte::from_u64(size)
                            .get_appropriate_unit(UnitType::Binary)
                            .to_string();
                        eprintln!("  {} {}", dim.apply_to("writes:  "), image_size);
                    }
                    eprintln!();

                    if !Confirm::with_theme(&theme)
                        .with_prompt("Overwrite this target?")
                        .default(false)
                        .interact()
                        .unwrap_or(false)
                    {
                        std::process::exit(0);
                    }
                } else {
                    // Non-TTY: just refuse without --confirm flag.
                    error!(
                        "Output '{}' already exists; pass --confirm to overwrite",
                        output
                    );
                    return ExitCode::FAILURE;
                }
            }

            let total = image_handle
                .protected_header
                .as_ref()
                .map(|h| h.cluster_count as usize)
                .unwrap_or(0);
            let block_size = image_handle
                .protected_header
                .as_ref()
                .map(|h| h.block_size as u64)
                .unwrap_or(0);
            if let Err(err) = image_handle.write(
                output_path,
                can_preload(image_handle.file_size),
                ProgressBar::Write.new_write(total, block_size),
            ) {
                error!(error = ?err, "Failed to write image");
                return ExitCode::FAILURE;
            }

            // Fixup backup GPT using the actual destination size.
            // BLKGETSIZE64 (0x80081272) is used for block devices on Linux;
            // regular files fall back to metadata.
            let mut dest_file = match OpenOptions::new().read(true).write(true).open(output_path) {
                Ok(f) => f,
                Err(err) => {
                    error!(error = ?err, "Failed to reopen output for GPT fixup");
                    return ExitCode::FAILURE;
                }
            };
            let dest_size = {
                use std::os::unix::fs::MetadataExt;
                use std::os::unix::io::AsRawFd;
                let mut size: u64 = 0;
                // BLKGETSIZE64 = 0x80081272 — returns u64 byte count
                let ret = unsafe {
                    libc::ioctl(dest_file.as_raw_fd(), 0x80081272u64, &mut size as *mut u64)
                };
                if ret == 0 && size > 0 {
                    size
                } else {
                    match dest_file.metadata() {
                        Ok(m) => m.size(),
                        Err(err) => {
                            error!(error = ?err, "Failed to get output size for GPT fixup");
                            return ExitCode::FAILURE;
                        }
                    }
                }
            };
            if let Err(err) = fixup_backup_gpt(&mut dest_file, dest_size) {
                error!(error = ?err, "Failed to fixup backup GPT");
                return ExitCode::FAILURE;
            }

            ExitCode::SUCCESS
        }
        _ => panic!(),
    }
}
