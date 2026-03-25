use anyhow::Result;
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
};
use tracing::{debug, trace};

/// Compute a CRC32 checksum using the IEEE polynomial (same as used by GPT).
fn crc32(data: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for i in 0..256u32 {
        let mut c = i;
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xedb88320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
        table[i as usize] = c;
    }
    let mut crc: u32 = 0xffffffff;
    for &b in data {
        crc = table[((crc ^ b as u32) & 0xff) as usize] ^ (crc >> 8);
    }
    crc ^ 0xffffffff
}

/// Read the primary GPT header from `dest`, construct the backup GPT header and
/// partition entries, then write them to the correct location at the end of the
/// disk.
///
/// `dest_size` is the byte size of the destination device or file, provided by
/// the caller (avoids a platform-specific ioctl inside the image library).
///
/// The backup GPT header is a copy of the primary with:
/// - `MyLBA` set to the last LBA of the disk
/// - `AlternateLBA` set to LBA 1 (the primary)
/// - `PartitionEntryLBA` set to `MyLBA - 32` (32 sectors before the backup header)
/// - `HeaderCRC32` recomputed (with the field zeroed while computing)
///
/// The backup partition entry array is an identical copy of the primary entries
/// placed immediately before the backup header.
pub fn fixup_backup_gpt(dest: &mut File, dest_size: u64) -> Result<()> {
    // ---- read the primary GPT header (LBA 1 = offset 512) ----
    dest.seek(SeekFrom::Start(512))?;
    let mut hdr = [0u8; 512];
    dest.read_exact(&mut hdr)?;

    // Verify GPT signature "EFI PART"
    if &hdr[0..8] != b"EFI PART" {
        trace!("No GPT signature found, skipping backup GPT fixup");
        return Ok(());
    }

    // Parse fields (all little-endian)
    let header_size = u32::from_le_bytes(hdr[12..16].try_into().unwrap()) as usize;
    let my_lba = u64::from_le_bytes(hdr[24..32].try_into().unwrap());
    let alternate_lba = u64::from_le_bytes(hdr[32..40].try_into().unwrap());
    let part_entry_lba = u64::from_le_bytes(hdr[72..80].try_into().unwrap());
    let num_entries = u32::from_le_bytes(hdr[80..84].try_into().unwrap());
    let entry_size = u32::from_le_bytes(hdr[84..88].try_into().unwrap()) as usize;

    // Sanity checks
    if my_lba != 1 || alternate_lba == 0 || header_size < 92 || entry_size == 0 || num_entries == 0
    {
        trace!(
            my_lba,
            alternate_lba, header_size, "GPT header looks invalid, skipping backup fixup"
        );
        return Ok(());
    }

    // ---- read the primary partition entries ----
    dest.seek(SeekFrom::Start(part_entry_lba * 512))?;
    let entries_size = num_entries as usize * entry_size;
    let mut entries = vec![0u8; entries_size];
    dest.read_exact(&mut entries)?;

    let disk_last_lba = dest_size / 512 - 1;
    let backup_entries_lba = disk_last_lba - 32;

    // LastUsableLBA = disk_last_lba - 33  (backup header + 32 entry sectors)
    let last_usable_lba = disk_last_lba - 33;

    // ---- update primary header to reflect actual disk geometry ----
    // AlternateLBA (bytes 32–40) and LastUsableLBA (bytes 48–56)
    let mut primary_hdr = hdr[..header_size].to_vec();
    primary_hdr[32..40].copy_from_slice(&disk_last_lba.to_le_bytes());
    primary_hdr[48..56].copy_from_slice(&last_usable_lba.to_le_bytes());
    primary_hdr[16..20].copy_from_slice(&[0u8; 4]);
    let primary_crc = crc32(&primary_hdr);
    primary_hdr[16..20].copy_from_slice(&primary_crc.to_le_bytes());
    let mut primary_sector = [0u8; 512];
    primary_sector[..primary_hdr.len()].copy_from_slice(&primary_hdr);
    dest.seek(SeekFrom::Start(512))?;
    dest.write_all(&primary_sector)?;

    // ---- build the backup header ----
    let mut backup_hdr = hdr[..header_size].to_vec();
    backup_hdr.resize(header_size, 0);

    // Swap MyLBA and AlternateLBA
    backup_hdr[24..32].copy_from_slice(&disk_last_lba.to_le_bytes());
    backup_hdr[32..40].copy_from_slice(&my_lba.to_le_bytes());

    // Update LastUsableLBA
    backup_hdr[48..56].copy_from_slice(&last_usable_lba.to_le_bytes());

    // Point PartitionEntryLBA to the backup entries location
    backup_hdr[72..80].copy_from_slice(&backup_entries_lba.to_le_bytes());

    // Recompute partition entries CRC32 (same entries, same data)
    let entries_crc = crc32(&entries);
    backup_hdr[88..92].copy_from_slice(&entries_crc.to_le_bytes());

    // Recompute header CRC32 (zero out the CRC field first)
    backup_hdr[16..20].copy_from_slice(&[0u8; 4]);
    let header_crc = crc32(&backup_hdr);
    backup_hdr[16..20].copy_from_slice(&header_crc.to_le_bytes());

    // ---- write backup partition entries ----
    dest.seek(SeekFrom::Start(backup_entries_lba * 512))?;
    dest.write_all(&entries)?;

    // ---- write backup header ----
    let mut backup_sector = [0u8; 512];
    backup_sector[..backup_hdr.len()].copy_from_slice(&backup_hdr);
    dest.seek(SeekFrom::Start(disk_last_lba * 512))?;
    dest.write_all(&backup_sector)?;

    debug!(disk_last_lba, backup_entries_lba, "Wrote backup GPT header");
    Ok(())
}
