//! Merges the disks of multiple image elements into a single multiboot
//! ("alloy") disk.
//!
//! Each element builds into its own qcow2, where the OS installer is free to
//! partition the whole disk. Afterwards the per-element disks are combined
//! into one GPT disk:
//!
//! - The first element's EFI system partition leads the disk and is copied
//!   verbatim, so its filesystem UUID, PARTUUID, and fallback bootloader
//!   (`EFI/BOOT/BOOTX64.EFI`) stay intact — the first element is the default
//!   boot target.
//! - All non-ESP partitions of every element follow in element order, with
//!   their type GUIDs, unique GUIDs (PARTUUIDs), attributes, and names
//!   preserved so `root=UUID=`/`root=PARTUUID=` references keep working.
//! - Later elements' ESP files are merged into the shared ESP; on conflict
//!   the earlier element wins.

use crate::gpt::{ESP_TYPE_GUID, Gpt, PartitionEntry, read_gpt, write_gpt};
use anyhow::{Context, Result, bail};
use goldboot_image::qcow::Qcow3;
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
    process::{Command, Stdio},
};
use tracing::{debug, info};

const SECTOR: u64 = 512;
/// Partitions are aligned to 1 MiB boundaries.
const ALIGN_SECTORS: u64 = 2048;

/// Merge the disks of all elements into a single alloy qcow2 at `dest`.
pub fn merge_qcows(sources: &[Qcow3], dest: &Path) -> Result<()> {
    let mut readers = sources
        .iter()
        .map(|qcow| qcow.reader())
        .collect::<std::io::Result<Vec<_>>>()?;

    let raw_path = dest.with_extension("raw");
    let result = merge_disks(&mut readers, &raw_path).and_then(|_| {
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

struct SourceDisk {
    gpt: Gpt,
    esp: PartitionEntry,
}

/// A planned copy of one source partition into the merged disk.
struct PartitionCopy {
    source: usize,
    source_first_lba: u64,
    dest_first_lba: u64,
    sectors: u64,
}

/// Merge multiple single-OS GPT disks into one raw disk image at `dest_path`.
/// Returns the size of the merged disk in bytes.
pub fn merge_disks<S: Read + Seek>(sources: &mut [S], dest_path: &Path) -> Result<u64> {
    let mut disks = Vec::new();
    for (i, source) in sources.iter_mut().enumerate() {
        let gpt = read_gpt(source)?
            .with_context(|| format!("element {i} has no GPT partition table"))?;
        let esp = gpt
            .entries
            .iter()
            .find(|e| e.type_guid == ESP_TYPE_GUID)
            .cloned()
            .with_context(|| format!("element {i} has no EFI system partition"))?;
        disks.push(SourceDisk { gpt, esp });
    }

    // Plan the merged layout: element 0's ESP first, then every non-ESP
    // partition in element order.
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

    let esp0 = disks[0].esp.clone();
    place(&mut entries, &mut copies, &mut next_lba, 0, &esp0);
    for (i, disk) in disks.iter().enumerate() {
        let mut parts: Vec<PartitionEntry> = disk
            .gpt
            .entries
            .iter()
            .filter(|e| e.index != disk.esp.index)
            .cloned()
            .collect();
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

    // Merge later elements' ESP files into the shared ESP (first element wins
    // on conflicts).
    let shared_esp = entries[0].clone();
    let temp_dir = dest_path.parent().unwrap_or(Path::new("."));
    for (i, disk) in disks.iter().enumerate().skip(1) {
        debug!(element = i, "Merging EFI system partition files");
        let mut esp_file = tempfile::tempfile_in(temp_dir)?;
        let esp_size = (disk.esp.last_lba - disk.esp.first_lba + 1) * SECTOR;
        esp_file.set_len(esp_size)?;
        copy_range(
            &mut sources[i],
            &mut esp_file,
            disk.esp.first_lba * SECTOR,
            0,
            esp_size,
        )?;
        merge_esp_files(&mut dest, &shared_esp, esp_file)
            .with_context(|| format!("failed to merge ESP files of element {i}"))?;
    }

    dest.sync_all()?;
    Ok(disk_size)
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

/// Copy files from the FAT filesystem in `src_esp_file` into the ESP at
/// `dest_esp` on the merged disk. Existing files are left alone.
fn merge_esp_files(
    dest: &mut File,
    dest_esp: &PartitionEntry,
    mut src_esp_file: File,
) -> Result<()> {
    // fatfs requires the stream to be positioned at the start
    src_esp_file.seek(SeekFrom::Start(0))?;
    let src_fs = fatfs::FileSystem::new(src_esp_file, fatfs::FsOptions::new())?;

    let slice = fscommon::StreamSlice::new(
        dest.try_clone()?,
        dest_esp.first_lba * SECTOR,
        (dest_esp.last_lba + 1) * SECTOR,
    )?;
    let dest_fs = fatfs::FileSystem::new(fscommon::BufStream::new(slice), fatfs::FsOptions::new())?;

    copy_dir_recursive(&src_fs.root_dir(), &dest_fs.root_dir())?;

    // Flush eagerly so write errors surface here instead of being swallowed
    // by the implicit unmount on drop.
    dest_fs.unmount()?;
    Ok(())
}

fn copy_dir_recursive<S, D>(src: &fatfs::Dir<'_, S>, dst: &fatfs::Dir<'_, D>) -> Result<()>
where
    S: fatfs::ReadWriteSeek,
    D: fatfs::ReadWriteSeek,
{
    for entry in src.iter() {
        let entry = entry?;
        let name = entry.file_name();
        if name == "." || name == ".." {
            continue;
        }
        if entry.is_dir() {
            let sub = dst.create_dir(&name)?;
            copy_dir_recursive(&entry.to_dir(), &sub)?;
        } else {
            // First element wins: skip files that already exist
            if dst.open_file(&name).is_ok() {
                debug!(name, "Skipping existing ESP file");
                continue;
            }
            let mut src_file = entry.to_file();
            let mut dst_file = dst.create_file(&name)?;
            std::io::copy(&mut src_file, &mut dst_file)?;
        }
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

        let tmp = tempfile::tempdir()?;
        let merged_path = tmp.path().join("merged.raw");
        let mut sources = vec![disk_a, disk_b];
        let disk_size = merge_disks(&mut sources, &merged_path)?;

        let mut merged = File::options().read(true).write(true).open(&merged_path)?;
        assert_eq!(merged.metadata()?.len(), disk_size);

        let gpt = read_gpt(&mut merged)?.expect("merged disk has no GPT");
        assert_eq!(gpt.entries.len(), 3);
        assert_eq!(gpt.entries[0].type_guid, ESP_TYPE_GUID);
        assert_eq!(gpt.entries[1].type_guid, LINUX_FS_GUID);
        assert_eq!(gpt.entries[2].type_guid, LINUX_FS_GUID);
        // Roots keep distinct PARTUUIDs and their original size
        assert_ne!(gpt.entries[1].unique_guid, gpt.entries[2].unique_guid);
        for entry in &gpt.entries[1..] {
            assert_eq!(entry.last_lba - entry.first_lba + 1, 4096);
        }

        // Partition contents were copied to their new locations
        for (entry, fill) in [(&gpt.entries[1], 0xAAu8), (&gpt.entries[2], 0xBBu8)] {
            let mut buf = vec![0u8; 4096];
            merged.seek(SeekFrom::Start(entry.first_lba * SECTOR))?;
            merged.read_exact(&mut buf)?;
            assert!(buf.iter().all(|&b| b == fill));
            merged.seek(SeekFrom::Start(entry.last_lba * SECTOR))?;
            let mut last = vec![0u8; 512];
            merged.read_exact(&mut last)?;
            assert!(last.iter().all(|&b| b == fill));
        }

        // Shared ESP: element 0's bootloader won, element 1's extra file merged
        let esp = &gpt.entries[0];
        let slice = fscommon::StreamSlice::new(
            merged.try_clone()?,
            esp.first_lba * SECTOR,
            (esp.last_lba + 1) * SECTOR,
        )?;
        let fs = fatfs::FileSystem::new(slice, fatfs::FsOptions::new())?;
        let root_dir = fs.root_dir();

        let mut content = Vec::new();
        root_dir
            .open_file("EFI/BOOT/BOOTX64.EFI")?
            .read_to_end(&mut content)?;
        assert_eq!(content, b"bootloader A");

        let mut content = Vec::new();
        root_dir
            .open_file("EFI/a/a.conf")?
            .read_to_end(&mut content)?;
        assert_eq!(content, b"config A");

        let mut content = Vec::new();
        root_dir
            .open_file("EFI/b/b.conf")?
            .read_to_end(&mut content)?;
        assert_eq!(content, b"config B");

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
        let err = merge_disks(&mut sources, &tmp.path().join("merged.raw")).unwrap_err();
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

        let merged_path = tmp.path().join("merged.qcow2");
        merge_qcows(&qcows, &merged_path)?;

        // The intermediate raw is cleaned up
        assert!(!merged_path.with_extension("raw").exists());

        let merged = Qcow3::open(&merged_path)?;
        let mut reader = merged.reader()?;
        let gpt = read_gpt(&mut reader)?.expect("merged qcow has no GPT");
        assert_eq!(gpt.entries.len(), 3);
        assert_eq!(gpt.entries[0].type_guid, ESP_TYPE_GUID);

        // Element 0's bootloader is the default
        let mut esp = vec![0u8; ((gpt.entries[0].last_lba + 1) * SECTOR) as usize];
        reader.seek(SeekFrom::Start(0))?;
        reader.read_exact(&mut esp)?;
        let esp_start = (gpt.entries[0].first_lba * SECTOR) as usize;
        assert!(
            esp[esp_start..]
                .windows(12)
                .any(|w| w == b"bootloader 0")
        );

        // Both root partitions arrived intact
        for (entry, fill) in [(&gpt.entries[1], 0xAAu8), (&gpt.entries[2], 0xBBu8)] {
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
        merge_disks(&mut sources, &merged_path)?;

        let mut merged = File::open(&merged_path)?;
        let gpt = read_gpt(&mut merged)?.expect("merged disk has no GPT");
        assert_eq!(gpt.entries.len(), 2);
        assert_eq!(gpt.entries[0].type_guid, ESP_TYPE_GUID);
        assert_eq!(gpt.entries[0].first_lba, 2048);
        Ok(())
    }
}
