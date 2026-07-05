//! Discovers chain-loadable bootloaders by scanning the EFI system
//! partitions of attached disks.
//!
//! This powers the boot-target selection screen shown when the system was
//! booted from goldboot.efi: each target can be chain-loaded by registering
//! a `Boot####` entry + `BootNext` for it (see [`chain_load`]) and rebooting.
//!
//! Goldboot's own ESP is excluded from the results, identified by the
//! `LoaderDevicePartUUID` EFI variable that systemd-stub sets, or — when
//! that variable is missing — by the GPT partition name written at merge
//! time ([`GOLDBOOT_ESP_NAME`]). If both mechanisms miss (e.g. goldboot.efi
//! was launched from a foreign ESP by unusual firmware), the worst case is
//! goldboot's own loader appearing as a menu entry, which merely re-enters
//! the menu when chosen.

use crate::boot::EspInfo;
use crate::gpt::{ESP_TYPE_GUID, read_gpt};
use anyhow::Result;
use std::{
    io::{Read, Seek, Write},
    path::Path,
};
use tracing::debug;
use uuid::Uuid;

/// GPT partition name of the ESP created for goldboot.efi on multiboot
/// (alloy) disks.
pub const GOLDBOOT_ESP_NAME: &str = "goldboot-esp";

/// efivarfs path of the ESP PARTUUID variable set by systemd-stub.
const LOADER_DEVICE_PART_UUID: &str =
    "/sys/firmware/efi/efivars/LoaderDevicePartUUID-4a67b082-0a4c-41cf-b6c7-440b29bb8c4f";

/// A bootloader found on some ESP that goldboot can chain-load.
#[derive(Debug, Clone)]
pub struct BootTarget {
    /// Whole-disk device name, e.g. "vda"
    pub disk: String,
    /// The ESP holding the loader, ready for `register_boot_entry`
    pub esp: EspInfo,
    /// Partition device name for display, e.g. "vda2" or "nvme0n1p2"
    pub part_dev: String,
    /// Vendor directory the loader was found in, e.g. "alpine" or "BOOT"
    pub loader_dir: String,
    /// Loader path in EFI notation, e.g. "\\EFI\\alpine\\grubx64.efi"
    pub efi_path: String,
    /// Human-readable menu label, e.g. "alpine (vda2)"
    pub label: String,
}

/// Scan all attached disks for chain-loadable bootloaders.
#[cfg(feature = "uki")]
pub fn scan_boot_targets() -> Vec<BootTarget> {
    let own_esp = own_esp_partuuid();
    debug!(own_esp = ?own_esp, "Scanning for boot targets");

    let mut devices = crate::gui::state::scan_block_devices();
    devices.sort_by(|a, b| a.name.cmp(&b.name));

    let mut targets = Vec::new();
    for device in devices {
        let path = format!("/dev/{}", device.name);
        let mut disk = match std::fs::File::options().read(true).open(&path) {
            Ok(f) => f,
            Err(e) => {
                debug!(device = %path, error = %e, "Cannot open disk for scanning");
                continue;
            }
        };
        match scan_disk(&mut disk, &device.name, own_esp, GOLDBOOT_ESP_NAME) {
            Ok(mut found) => targets.append(&mut found),
            Err(e) => debug!(device = %path, error = %e, "Failed to scan disk"),
        }
    }

    debug!(count = targets.len(), "Boot target scan complete");
    targets
}

/// Read the PARTUUID of the ESP goldboot.efi was loaded from, as recorded
/// by systemd-stub. Returns `None` on BIOS boots or when the variable is
/// missing. Reads efivarfs directly instead of `efivar::system()`, which
/// panics when no EFI variable store exists.
pub fn own_esp_partuuid() -> Option<Uuid> {
    let raw = std::fs::read(LOADER_DEVICE_PART_UUID).ok()?;
    // efivarfs prefixes the value with a 4-byte attribute header
    let utf16: Vec<u16> = raw
        .get(4..)?
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .take_while(|&c| c != 0)
        .collect();
    let text = String::from_utf16(&utf16).ok()?;
    Uuid::parse_str(text.trim()).ok()
}

/// Register a boot entry for the target and set `BootNext` to it. The
/// caller is expected to reboot afterwards; firmware then loads the target
/// bootloader once without altering the permanent boot order.
pub fn chain_load(target: &BootTarget) -> Result<u16> {
    if !Path::new("/sys/firmware/efi").exists() {
        anyhow::bail!("Cannot chain-load: not booted via EFI");
    }
    crate::boot::register_boot_entry(
        &target.esp,
        &format!("goldboot: {}", target.label),
        &target.efi_path,
    )
}

/// Scan one GPT disk for bootloaders on its ESPs. ESPs matching
/// `exclude_partuuid` are skipped; when it is `None`, ESPs whose GPT name is
/// `exclude_name` are skipped instead.
///
/// The `Write` bound comes from `fatfs`; nothing is ever written.
pub fn scan_disk<S: Read + Write + Seek>(
    disk: &mut S,
    disk_name: &str,
    exclude_partuuid: Option<Uuid>,
    exclude_name: &str,
) -> Result<Vec<BootTarget>> {
    const SECTOR: u64 = 512;

    let Some(gpt) = read_gpt(disk)? else {
        return Ok(Vec::new());
    };

    let mut targets = Vec::new();
    for entry in gpt.entries.iter().filter(|e| e.type_guid == ESP_TYPE_GUID) {
        let own = match exclude_partuuid {
            Some(uuid) => entry.unique_guid == uuid,
            None => entry.name == exclude_name,
        };
        if own {
            debug!(partition = entry.index, "Skipping goldboot's own ESP");
            continue;
        }

        let part_dev = partition_dev_name(disk_name, entry.index);
        let loaders = {
            let slice = fscommon::StreamSlice::new(
                &mut *disk,
                entry.first_lba * SECTOR,
                (entry.last_lba + 1) * SECTOR,
            )?;
            let fs = match fatfs::FileSystem::new(
                fscommon::BufStream::new(slice),
                fatfs::FsOptions::new(),
            ) {
                Ok(fs) => fs,
                Err(e) => {
                    debug!(partition = %part_dev, error = %e, "Skipping unreadable ESP");
                    continue;
                }
            };
            scan_esp_loaders(&fs)
        };

        for (loader_dir, file_name) in loaders {
            let label = if loader_dir.eq_ignore_ascii_case("BOOT") {
                format!("fallback ({part_dev})")
            } else {
                format!("{loader_dir} ({part_dev})")
            };
            targets.push(BootTarget {
                disk: disk_name.to_string(),
                esp: EspInfo {
                    partition_number: entry.index,
                    partition_start_lba: entry.first_lba,
                    partition_size_lba: entry.last_lba - entry.first_lba + 1,
                    partition_guid: entry.unique_guid,
                },
                part_dev: part_dev.clone(),
                efi_path: format!("\\EFI\\{loader_dir}\\{file_name}"),
                loader_dir,
                label,
            });
        }
    }
    Ok(targets)
}

/// Collect `EFI/*/*.efi` loaders from an ESP filesystem, vendor directories
/// before the `BOOT` fallback, alphabetical within each group. Names keep
/// their on-disk case.
fn scan_esp_loaders<T: fatfs::ReadWriteSeek>(fs: &fatfs::FileSystem<T>) -> Vec<(String, String)> {
    let mut loaders = Vec::new();

    let root = fs.root_dir();
    let Some(efi_dir) = root.iter().flatten().find(|e| {
        e.is_dir() && e.file_name().eq_ignore_ascii_case("EFI")
    }) else {
        return loaders;
    };

    for subdir in efi_dir.to_dir().iter().flatten() {
        let dir_name = subdir.file_name();
        if !subdir.is_dir() || dir_name == "." || dir_name == ".." {
            continue;
        }
        for file in subdir.to_dir().iter().flatten() {
            let file_name = file.file_name();
            if !file.is_dir() && file_name.to_ascii_lowercase().ends_with(".efi") {
                loaders.push((dir_name.clone(), file_name));
            }
        }
    }

    loaders.sort_by(|a, b| {
        let a_boot = a.0.eq_ignore_ascii_case("BOOT");
        let b_boot = b.0.eq_ignore_ascii_case("BOOT");
        a_boot
            .cmp(&b_boot)
            .then_with(|| a.0.to_ascii_lowercase().cmp(&b.0.to_ascii_lowercase()))
            .then_with(|| a.1.to_ascii_lowercase().cmp(&b.1.to_ascii_lowercase()))
    });
    loaders
}

/// Derive a partition device name from a whole-disk name and a 1-based
/// partition index: "vda" + 2 → "vda2", "nvme0n1" + 2 → "nvme0n1p2".
fn partition_dev_name(disk: &str, index: u32) -> String {
    if disk.ends_with(|c: char| c.is_ascii_digit()) {
        format!("{disk}p{index}")
    } else {
        format!("{disk}{index}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gpt::{PartitionEntry, write_gpt};
    use std::io::{Cursor, SeekFrom};

    const SECTOR: u64 = 512;
    const MIB: u64 = 1024 * 1024;

    /// Build a synthetic GPT disk with FAT-formatted ESPs at consecutive
    /// 2 MiB partitions, each populated with the given files.
    fn synthetic_disk(
        esps: &[(&str, Option<Uuid>, &[(&str, &[u8])])],
    ) -> Result<Cursor<Vec<u8>>> {
        let disk_size = 4 * MIB + esps.len() as u64 * 2 * MIB;
        let mut entries = Vec::new();
        for (i, (name, uuid, _)) in esps.iter().enumerate() {
            let first_lba = 2048 + i as u64 * 4096;
            entries.push(PartitionEntry {
                index: i as u32 + 1,
                type_guid: ESP_TYPE_GUID,
                unique_guid: uuid.unwrap_or_else(Uuid::new_v4),
                first_lba,
                last_lba: first_lba + 4095,
                attributes: 0,
                name: name.to_string(),
            });
        }

        let mut disk = Cursor::new(vec![0u8; disk_size as usize]);
        write_gpt(&mut disk, disk_size, &entries)?;

        for (entry, (_, _, files)) in entries.iter().zip(esps.iter()) {
            let mut slice = fscommon::StreamSlice::new(
                &mut disk,
                entry.first_lba * SECTOR,
                (entry.last_lba + 1) * SECTOR,
            )?;
            fatfs::format_volume(&mut slice, fatfs::FormatVolumeOptions::new())?;
            let fs = fatfs::FileSystem::new(&mut slice, fatfs::FsOptions::new())?;
            {
                let root = fs.root_dir();
                for (path, content) in files.iter() {
                    let components: Vec<&str> = path.split('/').collect();
                    let mut dir = root.clone();
                    for component in &components[..components.len() - 1] {
                        dir = dir.create_dir(component)?;
                    }
                    let mut file = dir.create_file(components.last().unwrap())?;
                    file.write_all(content)?;
                }
            }
            fs.unmount()?;
        }

        disk.seek(SeekFrom::Start(0))?;
        Ok(disk)
    }

    #[test]
    fn scan_finds_loaders_and_orders_vendor_dirs_first() -> Result<()> {
        let mut disk = synthetic_disk(&[(
            "esp",
            None,
            &[
                ("EFI/BOOT/BOOTX64.EFI", b"fallback".as_slice()),
                ("EFI/alpine/grubx64.efi", b"grub".as_slice()),
                ("EFI/alpine/README.txt", b"not a loader".as_slice()),
            ],
        )])?;

        let targets = scan_disk(&mut disk, "vda", None, GOLDBOOT_ESP_NAME)?;
        assert_eq!(targets.len(), 2);
        assert_eq!(targets[0].efi_path, "\\EFI\\alpine\\grubx64.efi");
        assert_eq!(targets[0].label, "alpine (vda1)");
        assert_eq!(targets[1].efi_path, "\\EFI\\BOOT\\BOOTX64.EFI");
        assert_eq!(targets[1].label, "fallback (vda1)");
        assert_eq!(targets[0].esp.partition_number, 1);
        Ok(())
    }

    #[test]
    fn scan_matches_efi_names_case_insensitively() -> Result<()> {
        let mut disk = synthetic_disk(&[(
            "esp",
            None,
            &[("efi/debian/shimx64.EFI", b"shim".as_slice())],
        )])?;

        let targets = scan_disk(&mut disk, "nvme0n1", None, GOLDBOOT_ESP_NAME)?;
        assert_eq!(targets.len(), 1);
        // On-disk case is preserved in the loader path
        assert_eq!(targets[0].efi_path, "\\EFI\\debian\\shimx64.EFI");
        assert_eq!(targets[0].part_dev, "nvme0n1p1");
        Ok(())
    }

    #[test]
    fn scan_excludes_own_esp_by_partuuid_or_name() -> Result<()> {
        let own_uuid = Uuid::new_v4();
        let esps: &[(&str, Option<Uuid>, &[(&str, &[u8])])] = &[
            (
                GOLDBOOT_ESP_NAME,
                Some(own_uuid),
                &[("EFI/BOOT/BOOTX64.EFI", b"goldboot".as_slice())],
            ),
            (
                "esp",
                None,
                &[("EFI/alpine/grubx64.efi", b"grub".as_slice())],
            ),
        ];

        // Excluded by PARTUUID
        let mut disk = synthetic_disk(esps)?;
        let targets = scan_disk(&mut disk, "vda", Some(own_uuid), GOLDBOOT_ESP_NAME)?;
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].loader_dir, "alpine");

        // Excluded by GPT name when no PARTUUID is known
        let mut disk = synthetic_disk(esps)?;
        let targets = scan_disk(&mut disk, "vda", None, GOLDBOOT_ESP_NAME)?;
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].loader_dir, "alpine");

        // Not excluded when neither matches
        let mut disk = synthetic_disk(esps)?;
        let targets = scan_disk(&mut disk, "vda", Some(Uuid::new_v4()), GOLDBOOT_ESP_NAME)?;
        assert_eq!(targets.len(), 2);
        Ok(())
    }

    #[test]
    fn scan_skips_unformatted_esp() -> Result<()> {
        // GPT with an ESP entry but no FAT filesystem inside
        let disk_size = 8 * MIB;
        let mut disk = Cursor::new(vec![0u8; disk_size as usize]);
        write_gpt(
            &mut disk,
            disk_size,
            &[PartitionEntry {
                index: 1,
                type_guid: ESP_TYPE_GUID,
                unique_guid: Uuid::new_v4(),
                first_lba: 2048,
                last_lba: 6143,
                attributes: 0,
                name: "esp".to_string(),
            }],
        )?;
        disk.seek(SeekFrom::Start(0))?;

        let targets = scan_disk(&mut disk, "vda", None, GOLDBOOT_ESP_NAME)?;
        assert!(targets.is_empty());
        Ok(())
    }

    #[test]
    fn partition_dev_names() {
        assert_eq!(partition_dev_name("vda", 2), "vda2");
        assert_eq!(partition_dev_name("sdb", 1), "sdb1");
        assert_eq!(partition_dev_name("nvme0n1", 2), "nvme0n1p2");
        assert_eq!(partition_dev_name("mmcblk0", 3), "mmcblk0p3");
    }
}
