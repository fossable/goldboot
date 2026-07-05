//! Merges the disks of multiple image elements into a single multiboot
//! ("alloy") disk.
//!
//! Each element builds into its own qcow2, where the OS installer is free to
//! partition the whole disk. Afterwards the per-element disks are combined
//! into one GPT disk:
//!
//! - Partition 1 is a fresh "goldboot ESP" holding goldboot.efi at the
//!   firmware fallback path (e.g. `EFI/BOOT/BOOTX64.EFI`), so firmware boots
//!   the goldboot chain-loader menu by default.
//! - Every partition of every element follows in element order — including
//!   each element's own ESP — with type GUIDs, unique GUIDs (PARTUUIDs),
//!   attributes, names, and filesystem contents preserved verbatim. This
//!   keeps `root=UUID=`/`root=PARTUUID=` references and fstab ESP mounts
//!   working, and lets goldboot.efi chain-load each element's bootloader
//!   from its own ESP.

use crate::boot_scan::GOLDBOOT_ESP_NAME;
use crate::gpt::{ESP_TYPE_GUID, Gpt, PartitionEntry, read_gpt, write_gpt};
use anyhow::{Context, Result, bail};
use goldboot_image::{ImageArch, qcow::Qcow3};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
    process::{Command, Stdio},
};
use tracing::{debug, info};
use uuid::Uuid;

const SECTOR: u64 = 512;
/// Partitions are aligned to 1 MiB boundaries.
const ALIGN_SECTORS: u64 = 2048;

/// Firmware fallback loader filename for the given architecture (the alloy
/// disk's boot architecture is decided at build time, not compile time).
fn fallback_boot_file(arch: ImageArch) -> Result<&'static str> {
    Ok(match arch {
        ImageArch::Amd64 => "BOOTX64.EFI",
        ImageArch::Arm64 => "BOOTAA64.EFI",
        ImageArch::I386 => "BOOTIA32.EFI",
        _ => bail!("Unsupported architecture for multiboot: {arch:?}"),
    })
}

/// Merge the disks of all elements into a single alloy qcow2 at `dest`,
/// with `goldboot_efi` installed as the default (chain-loader) bootloader.
pub fn merge_qcows(sources: &[Qcow3], dest: &Path, goldboot_efi: &Path, arch: ImageArch) -> Result<()> {
    let mut readers = sources
        .iter()
        .map(|qcow| qcow.reader())
        .collect::<std::io::Result<Vec<_>>>()?;

    let efi_bytes = std::fs::read(goldboot_efi)
        .with_context(|| format!("failed to read {}", goldboot_efi.display()))?;
    let boot_file_name = fallback_boot_file(arch)?;

    let raw_path = dest.with_extension("raw");
    let result = merge_disks(&mut readers, &raw_path, &efi_bytes, boot_file_name).and_then(|_| {
        debug!(dest = %dest.display(), "Converting merged disk to qcow2");
        let status = Command::new("qemu-img")
            .args([
                "convert",
                "-f",
                "raw",
                "-O",
                "qcow2",
                "-o",
                "cluster_size=65536",
            ])
            .arg(&raw_path)
            .arg(dest)
            .stdout(Stdio::null())
            .status()
            .context("failed to run qemu-img")?;
        if !status.success() {
            bail!("qemu-img convert failed");
        }
        Ok(())
    });
    let _ = std::fs::remove_file(&raw_path);
    result
}

/// A planned copy of one source partition into the merged disk.
struct PartitionCopy {
    source: usize,
    source_first_lba: u64,
    dest_first_lba: u64,
    sectors: u64,
}

/// Merge multiple single-OS GPT disks into one raw disk image at `dest_path`.
/// `goldboot_efi` is written to `EFI/BOOT/<boot_file_name>` on a fresh ESP
/// leading the disk. Returns the size of the merged disk in bytes.
pub fn merge_disks<S: Read + Seek>(
    sources: &mut [S],
    dest_path: &Path,
    goldboot_efi: &[u8],
    boot_file_name: &str,
) -> Result<u64> {
    let mut disks: Vec<Gpt> = Vec::new();
    for (i, source) in sources.iter_mut().enumerate() {
        let gpt = read_gpt(source)?
            .with_context(|| format!("element {i} has no GPT partition table"))?;
        if !gpt.entries.iter().any(|e| e.type_guid == ESP_TYPE_GUID) {
            bail!("element {i} has no EFI system partition");
        }
        disks.push(gpt);
    }

    // Plan the merged layout: a fresh goldboot ESP first, then every
    // partition of every element in element order.
    let mut entries: Vec<PartitionEntry> = Vec::new();
    let mut copies: Vec<PartitionCopy> = Vec::new();
    let mut next_lba = ALIGN_SECTORS;

    let place = |entries: &mut Vec<PartitionEntry>,
                     copies: &mut Vec<PartitionCopy>,
                     next_lba: &mut u64,
                     source: usize,
                     entry: &PartitionEntry| {
        let sectors = entry.last_lba - entry.first_lba + 1;
        let first = *next_lba;
        entries.push(PartitionEntry {
            index: entries.len() as u32 + 1,
            first_lba: first,
            last_lba: first + sectors - 1,
            ..entry.clone()
        });
        copies.push(PartitionCopy {
            source,
            source_first_lba: entry.first_lba,
            dest_first_lba: first,
            sectors,
        });
        *next_lba = (first + sectors).div_ceil(ALIGN_SECTORS) * ALIGN_SECTORS;
    };

    // Size the goldboot ESP for the EFI binary plus FAT overhead and slack.
    let esp_bytes = (goldboot_efi.len() as u64 + goldboot_efi.len() as u64 / 20 + 4 * 1024 * 1024)
        .max(16 * 1024 * 1024);
    let esp_sectors = esp_bytes.div_ceil(SECTOR).div_ceil(ALIGN_SECTORS) * ALIGN_SECTORS;
    let goldboot_esp = PartitionEntry {
        index: 1,
        type_guid: ESP_TYPE_GUID,
        unique_guid: Uuid::new_v4(),
        first_lba: next_lba,
        last_lba: next_lba + esp_sectors - 1,
        attributes: 0,
        name: GOLDBOOT_ESP_NAME.into(),
    };
    entries.push(goldboot_esp.clone());
    next_lba = (goldboot_esp.last_lba + 1).div_ceil(ALIGN_SECTORS) * ALIGN_SECTORS;

    for (i, disk) in disks.iter().enumerate() {
        let mut parts: Vec<PartitionEntry> = disk.entries.to_vec();
        parts.sort_by_key(|e| e.first_lba);
        for entry in parts {
            place(&mut entries, &mut copies, &mut next_lba, i, &entry);
        }
    }

    // Room for the backup GPT (33 sectors) after the last partition.
    let total_sectors = (entries.last().unwrap().last_lba + 34).div_ceil(ALIGN_SECTORS)
        * ALIGN_SECTORS;
    let disk_size = total_sectors * SECTOR;

    info!(
        dest = %dest_path.display(),
        size = disk_size,
        partitions = entries.len(),
        "Merging element disks"
    );

    let mut dest = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(dest_path)?;
    dest.set_len(disk_size)?;

    write_gpt(&mut dest, disk_size, &entries)?;

    for copy in &copies {
        debug!(
            source = copy.source,
            source_first_lba = copy.source_first_lba,
            dest_first_lba = copy.dest_first_lba,
            sectors = copy.sectors,
            "Copying partition"
        );
        copy_range(
            &mut sources[copy.source],
            &mut dest,
            copy.source_first_lba * SECTOR,
            copy.dest_first_lba * SECTOR,
            copy.sectors * SECTOR,
        )?;
    }

    write_goldboot_esp(&mut dest, &goldboot_esp, goldboot_efi, boot_file_name)
        .context("failed to populate the goldboot ESP")?;

    dest.sync_all()?;
    Ok(disk_size)
}

/// Format the goldboot ESP region and write the EFI binary to the firmware
/// fallback path `EFI/BOOT/<boot_file_name>`.
fn write_goldboot_esp(
    dest: &mut File,
    esp: &PartitionEntry,
    goldboot_efi: &[u8],
    boot_file_name: &str,
) -> Result<()> {
    debug!(
        first_lba = esp.first_lba,
        last_lba = esp.last_lba,
        efi_size = goldboot_efi.len(),
        "Formatting goldboot ESP"
    );

    let slice = fscommon::StreamSlice::new(
        dest.try_clone()?,
        esp.first_lba * SECTOR,
        (esp.last_lba + 1) * SECTOR,
    )?;
    let mut stream = fscommon::BufStream::new(slice);
    // Let fatfs pick the FAT type; forcing FAT32 fails on small volumes.
    fatfs::format_volume(
        &mut stream,
        fatfs::FormatVolumeOptions::new().volume_label(*b"GOLDBOOT   "),
    )?;

    let fs = fatfs::FileSystem::new(stream, fatfs::FsOptions::new())?;
    {
        let boot_dir = fs.root_dir().create_dir("EFI")?.create_dir("BOOT")?;
        let mut file = boot_dir.create_file(boot_file_name)?;
        for chunk in goldboot_efi.chunks(1 << 20) {
            file.write_all(chunk)?;
        }
    }

    // Flush eagerly so write errors surface here instead of being swallowed
    // by the implicit unmount on drop.
    fs.unmount()?;
    Ok(())
}

/// Copy `len` bytes from `src_offset` in `src` to `dst_offset` in `dst`,
/// skipping all-zero chunks to keep the destination sparse. The destination
/// range is assumed to contain zeros already.
fn copy_range<S: Read + Seek, D: Write + Seek>(
    src: &mut S,
    dst: &mut D,
    src_offset: u64,
    dst_offset: u64,
    len: u64,
) -> Result<()> {
    const CHUNK: usize = 1 << 20;

    src.seek(SeekFrom::Start(src_offset))?;
    let mut buf = vec![0u8; CHUNK];
    let mut pos = 0u64;
    while pos < len {
        let n = CHUNK.min((len - pos) as usize);
        src.read_exact(&mut buf[..n])?;
        if buf[..n].iter().any(|&b| b != 0) {
            dst.seek(SeekFrom::Start(dst_offset + pos))?;
            dst.write_all(&buf[..n])?;
        }
        pos += n as u64;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use uuid::Uuid;

    const LINUX_FS_GUID: Uuid = Uuid::from_u128(0x0FC63DAF_8483_4772_8E79_3D69D8477DE4);
    const MIB: u64 = 1024 * 1024;
    const FAKE_UKI: &[u8] = b"fake goldboot uki";

    /// Read a file from the FAT filesystem inside the given partition of a
    /// merged disk. Returns `None` if the file doesn't exist.
    fn read_fat_file(disk: &File, part: &PartitionEntry, path: &str) -> Result<Option<Vec<u8>>> {
        let slice = fscommon::StreamSlice::new(
            disk.try_clone()?,
            part.first_lba * SECTOR,
            (part.last_lba + 1) * SECTOR,
        )?;
        let fs = fatfs::FileSystem::new(fscommon::BufStream::new(slice), fatfs::FsOptions::new())?;
        let content = match fs.root_dir().open_file(path) {
            Ok(mut f) => {
                let mut content = Vec::new();
                f.read_to_end(&mut content)?;
                Some(content)
            }
            Err(_) => None,
        };
        Ok(content)
    }

    /// Build a 16 MiB synthetic UEFI disk: a 2 MiB FAT ESP containing
    /// `esp_files`, and a 2 MiB "root" partition filled with `fill`.
    fn synthetic_disk(esp_files: &[(&str, &[u8])], fill: u8) -> Result<Cursor<Vec<u8>>> {
        let disk_size = 16 * MIB;
        let esp = PartitionEntry {
            index: 1,
            type_guid: ESP_TYPE_GUID,
            unique_guid: Uuid::new_v4(),
            first_lba: 2048,
            last_lba: 6143,
            attributes: 0,
            name: "esp".into(),
        };
        let root = PartitionEntry {
            index: 2,
            type_guid: LINUX_FS_GUID,
            unique_guid: Uuid::new_v4(),
            first_lba: 6144,
            last_lba: 10239,
            attributes: 0,
            name: "root".into(),
        };

        let mut disk = Cursor::new(vec![0u8; disk_size as usize]);
        write_gpt(&mut disk, disk_size, &[esp.clone(), root.clone()])?;

        // Format and populate the ESP
        {
            let mut slice = fscommon::StreamSlice::new(
                &mut disk,
                esp.first_lba * SECTOR,
                (esp.last_lba + 1) * SECTOR,
            )?;
            fatfs::format_volume(&mut slice, fatfs::FormatVolumeOptions::new())?;
            let fs = fatfs::FileSystem::new(&mut slice, fatfs::FsOptions::new())?;
            let root_dir = fs.root_dir();
            for (path, content) in esp_files {
                let components: Vec<&str> = path.split('/').collect();
                let mut dir = root_dir.clone();
                for component in &components[..components.len() - 1] {
                    dir = dir.create_dir(component)?;
                }
                let mut file = dir.create_file(components.last().unwrap())?;
                file.write_all(content)?;
            }
        }

        // Fill the root partition with a recognizable pattern
        disk.seek(SeekFrom::Start(root.first_lba * SECTOR))?;
        let pattern = vec![fill; ((root.last_lba - root.first_lba + 1) * SECTOR) as usize];
        disk.write_all(&pattern)?;

        disk.seek(SeekFrom::Start(0))?;
        Ok(disk)
    }

    fn read_esp_file(fs_file: File, path: &str) -> Result<Option<Vec<u8>>> {
        let fs = fatfs::FileSystem::new(fs_file, fatfs::FsOptions::new())?;
        match fs.root_dir().open_file(path) {
            Ok(mut f) => {
                let mut content = Vec::new();
                f.read_to_end(&mut content)?;
                Ok(Some(content))
            }
            Err(_) => Ok(None),
        }
    }

    #[test]
    fn merge_two_disks() -> Result<()> {
        let disk_a = synthetic_disk(
            &[
                ("EFI/BOOT/BOOTX64.EFI", b"bootloader A".as_slice()),
                ("EFI/a/a.conf", b"config A".as_slice()),
            ],
            0xAA,
        )?;
        let disk_b = synthetic_disk(
            &[
                ("EFI/BOOT/BOOTX64.EFI", b"bootloader B".as_slice()),
                ("EFI/b/b.conf", b"config B".as_slice()),
            ],
            0xBB,
        )?;

        // Capture the source ESP PARTUUIDs to verify they survive the merge
        let mut source_esp_guids = Vec::new();
        for disk in [&disk_a, &disk_b] {
            let gpt = read_gpt(&mut disk.clone())?.unwrap();
            source_esp_guids.push(gpt.entries[0].unique_guid);
        }

        let tmp = tempfile::tempdir()?;
        let merged_path = tmp.path().join("merged.raw");
        let mut sources = vec![disk_a, disk_b];
        let disk_size = merge_disks(&mut sources, &merged_path, FAKE_UKI, "BOOTX64.EFI")?;

        let mut merged = File::options().read(true).write(true).open(&merged_path)?;
        assert_eq!(merged.metadata()?.len(), disk_size);

        let gpt = read_gpt(&mut merged)?.expect("merged disk has no GPT");
        assert_eq!(gpt.entries.len(), 5);

        // Partition 1 is the fresh goldboot ESP with the chain-loader
        assert_eq!(gpt.entries[0].type_guid, ESP_TYPE_GUID);
        assert_eq!(gpt.entries[0].name, GOLDBOOT_ESP_NAME);
        assert_eq!(
            read_fat_file(&merged, &gpt.entries[0], "EFI/BOOT/BOOTX64.EFI")?.as_deref(),
            Some(FAKE_UKI)
        );

        // Element ESPs are verbatim: original PARTUUIDs and unmerged contents
        for (i, (esp, bootloader)) in [(&gpt.entries[1], b"bootloader A".as_slice()),
            (&gpt.entries[3], b"bootloader B".as_slice())]
        .iter()
        .enumerate()
        {
            assert_eq!(esp.type_guid, ESP_TYPE_GUID);
            assert_eq!(esp.unique_guid, source_esp_guids[i]);
            assert_eq!(
                read_fat_file(&merged, esp, "EFI/BOOT/BOOTX64.EFI")?.as_deref(),
                Some(*bootloader)
            );
        }
        assert!(read_fat_file(&merged, &gpt.entries[1], "EFI/a/a.conf")?.is_some());
        assert!(read_fat_file(&merged, &gpt.entries[1], "EFI/b/b.conf")?.is_none());
        assert!(read_fat_file(&merged, &gpt.entries[3], "EFI/b/b.conf")?.is_some());
        assert!(read_fat_file(&merged, &gpt.entries[3], "EFI/a/a.conf")?.is_none());

        // Roots keep distinct PARTUUIDs and their original size
        assert_eq!(gpt.entries[2].type_guid, LINUX_FS_GUID);
        assert_eq!(gpt.entries[4].type_guid, LINUX_FS_GUID);
        assert_ne!(gpt.entries[2].unique_guid, gpt.entries[4].unique_guid);
        for entry in [&gpt.entries[2], &gpt.entries[4]] {
            assert_eq!(entry.last_lba - entry.first_lba + 1, 4096);
        }

        // Partition contents were copied to their new locations
        for (entry, fill) in [(&gpt.entries[2], 0xAAu8), (&gpt.entries[4], 0xBBu8)] {
            let mut buf = vec![0u8; 4096];
            merged.seek(SeekFrom::Start(entry.first_lba * SECTOR))?;
            merged.read_exact(&mut buf)?;
            assert!(buf.iter().all(|&b| b == fill));
            merged.seek(SeekFrom::Start(entry.last_lba * SECTOR))?;
            let mut last = vec![0u8; 512];
            merged.read_exact(&mut last)?;
            assert!(last.iter().all(|&b| b == fill));
        }

        Ok(())
    }

    #[test]
    fn merge_rejects_disk_without_esp() -> Result<()> {
        let disk_size = 16 * MIB;
        let root = PartitionEntry {
            index: 1,
            type_guid: LINUX_FS_GUID,
            unique_guid: Uuid::new_v4(),
            first_lba: 2048,
            last_lba: 6143,
            attributes: 0,
            name: "root".into(),
        };
        let mut disk = Cursor::new(vec![0u8; disk_size as usize]);
        write_gpt(&mut disk, disk_size, &[root])?;
        disk.seek(SeekFrom::Start(0))?;

        let ok_disk = synthetic_disk(&[("EFI/BOOT/BOOTX64.EFI", b"a".as_slice())], 0xAA)?;

        let tmp = tempfile::tempdir()?;
        let mut sources = vec![ok_disk, disk];
        let err = merge_disks(
            &mut sources,
            &tmp.path().join("merged.raw"),
            FAKE_UKI,
            "BOOTX64.EFI",
        )
        .unwrap_err();
        assert!(err.to_string().contains("no EFI system partition"));
        Ok(())
    }

    #[test]
    fn merge_qcows_end_to_end() -> Result<()> {
        let tmp = tempfile::tempdir()?;

        // Convert two synthetic raw disks into qcow2 sources
        let mut qcows = Vec::new();
        for (i, fill) in [0xAAu8, 0xBBu8].iter().enumerate() {
            let disk = synthetic_disk(
                &[("EFI/BOOT/BOOTX64.EFI", format!("bootloader {i}").as_bytes())],
                *fill,
            )?;
            let raw_path = tmp.path().join(format!("{i}.raw"));
            std::fs::write(&raw_path, disk.into_inner())?;
            let qcow_path = tmp.path().join(format!("{i}.qcow2"));
            let status = Command::new("qemu-img")
                .args(["convert", "-f", "raw", "-O", "qcow2"])
                .arg(&raw_path)
                .arg(&qcow_path)
                .status()?;
            assert!(status.success());
            qcows.push(Qcow3::open(&qcow_path)?);
        }

        let uki_path = tmp.path().join("goldboot.efi");
        std::fs::write(&uki_path, FAKE_UKI)?;

        let merged_path = tmp.path().join("merged.qcow2");
        merge_qcows(&qcows, &merged_path, &uki_path, ImageArch::Amd64)?;

        // The intermediate raw is cleaned up
        assert!(!merged_path.with_extension("raw").exists());

        let merged = Qcow3::open(&merged_path)?;
        let mut reader = merged.reader()?;
        let gpt = read_gpt(&mut reader)?.expect("merged qcow has no GPT");
        assert_eq!(gpt.entries.len(), 5);
        assert_eq!(gpt.entries[0].type_guid, ESP_TYPE_GUID);
        assert_eq!(gpt.entries[0].name, GOLDBOOT_ESP_NAME);

        // The chain-loader landed on the goldboot ESP and element 0's own
        // bootloader is intact on its ESP
        for (entry, needle) in [
            (&gpt.entries[0], FAKE_UKI),
            (&gpt.entries[1], b"bootloader 0".as_slice()),
        ] {
            let mut esp = vec![0u8; ((entry.last_lba - entry.first_lba + 1) * SECTOR) as usize];
            reader.seek(SeekFrom::Start(entry.first_lba * SECTOR))?;
            reader.read_exact(&mut esp)?;
            assert!(esp.windows(needle.len()).any(|w| w == needle));
        }

        // Both root partitions arrived intact
        for (entry, fill) in [(&gpt.entries[2], 0xAAu8), (&gpt.entries[4], 0xBBu8)] {
            let mut buf = vec![0u8; 4096];
            reader.seek(SeekFrom::Start(entry.first_lba * SECTOR))?;
            reader.read_exact(&mut buf)?;
            assert!(buf.iter().all(|&b| b == fill));
        }
        Ok(())
    }

    #[test]
    fn merge_single_disk_relocates_partitions() -> Result<()> {
        let disk = synthetic_disk(&[("EFI/BOOT/BOOTX64.EFI", b"a".as_slice())], 0xCC)?;
        let tmp = tempfile::tempdir()?;
        let merged_path = tmp.path().join("merged.raw");
        let mut sources = vec![disk];
        merge_disks(&mut sources, &merged_path, FAKE_UKI, "BOOTX64.EFI")?;

        let mut merged = File::open(&merged_path)?;
        let gpt = read_gpt(&mut merged)?.expect("merged disk has no GPT");
        assert_eq!(gpt.entries.len(), 3);
        assert_eq!(gpt.entries[0].type_guid, ESP_TYPE_GUID);
        assert_eq!(gpt.entries[0].name, GOLDBOOT_ESP_NAME);
        assert_eq!(gpt.entries[0].first_lba, 2048);
        assert_eq!(gpt.entries[1].type_guid, ESP_TYPE_GUID);
        assert_eq!(gpt.entries[2].type_guid, LINUX_FS_GUID);
        Ok(())
    }
}
