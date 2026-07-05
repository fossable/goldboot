use anyhow::Result;
use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
};
use tracing::{debug, info, trace};
use uuid::Uuid;

/// EFI System Partition type GUID.
pub const ESP_TYPE_GUID: Uuid = Uuid::from_u128(0xC12A7328_F81F_11D2_BA4B_00A0C93EC93B);

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

/// Parsed primary GPT header fields plus the raw 512-byte sector.
#[derive(Clone)]
pub struct GptHeader {
    pub header_size: usize,
    pub my_lba: u64,
    pub alternate_lba: u64,
    pub first_usable_lba: u64,
    pub last_usable_lba: u64,
    pub part_entry_lba: u64,
    pub num_entries: u32,
    pub entry_size: usize,
    /// Raw header sector (512 bytes) as read from disk.
    pub raw_sector: [u8; 512],
}

/// Parsed GPT partition entry.
#[derive(Clone, Debug)]
pub struct PartitionEntry {
    /// 1-based partition number (index in the entries array + 1).
    pub index: u32,
    pub type_guid: Uuid,
    pub unique_guid: Uuid,
    pub first_lba: u64,
    pub last_lba: u64,
    pub attributes: u64,
    pub name: String,
}

impl PartitionEntry {
    pub fn is_used(&self) -> bool {
        self.type_guid != Uuid::nil()
    }
}

/// Parsed GPT: primary header, entries (only used ones), and the raw entries blob.
pub struct Gpt {
    pub header: GptHeader,
    pub entries: Vec<PartitionEntry>,
    /// Raw entries region (num_entries * entry_size bytes) — preserved so callers
    /// can mutate one entry and re-CRC the whole region.
    pub entries_raw: Vec<u8>,
}

/// GPT type GUIDs are stored on disk in mixed-endian form (first three fields
/// little-endian, last two big-endian). Convert to a `Uuid`.
fn guid_from_disk(buf: &[u8; 16]) -> Uuid {
    let d1 = u32::from_le_bytes(buf[0..4].try_into().unwrap());
    let d2 = u16::from_le_bytes(buf[4..6].try_into().unwrap());
    let d3 = u16::from_le_bytes(buf[6..8].try_into().unwrap());
    let mut tail = [0u8; 8];
    tail.copy_from_slice(&buf[8..16]);
    Uuid::from_fields(d1, d2, d3, &tail)
}

/// Inverse of `guid_from_disk`.
fn guid_to_disk(uuid: &Uuid) -> [u8; 16] {
    let (d1, d2, d3, tail) = uuid.as_fields();
    let mut out = [0u8; 16];
    out[0..4].copy_from_slice(&d1.to_le_bytes());
    out[4..6].copy_from_slice(&d2.to_le_bytes());
    out[6..8].copy_from_slice(&d3.to_le_bytes());
    out[8..16].copy_from_slice(tail);
    out
}

fn parse_entry(index: u32, raw: &[u8]) -> PartitionEntry {
    let type_guid = guid_from_disk(raw[0..16].try_into().unwrap());
    let unique_guid = guid_from_disk(raw[16..32].try_into().unwrap());
    let first_lba = u64::from_le_bytes(raw[32..40].try_into().unwrap());
    let last_lba = u64::from_le_bytes(raw[40..48].try_into().unwrap());
    let attributes = u64::from_le_bytes(raw[48..56].try_into().unwrap());
    let name = {
        let utf16: Vec<u16> = raw[56..]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .take_while(|&c| c != 0)
            .collect();
        String::from_utf16_lossy(&utf16)
    };
    PartitionEntry {
        index,
        type_guid,
        unique_guid,
        first_lba,
        last_lba,
        attributes,
        name,
    }
}

/// Read and parse the primary GPT from `dest`. Returns `Ok(None)` if the GPT
/// signature is missing or the header fails basic sanity checks.
pub fn read_gpt<D: Read + Seek>(dest: &mut D) -> Result<Option<Gpt>> {
    dest.seek(SeekFrom::Start(512))?;
    let mut hdr = [0u8; 512];
    dest.read_exact(&mut hdr)?;

    if &hdr[0..8] != b"EFI PART" {
        trace!("No GPT signature found");
        return Ok(None);
    }

    let header_size = u32::from_le_bytes(hdr[12..16].try_into().unwrap()) as usize;
    let my_lba = u64::from_le_bytes(hdr[24..32].try_into().unwrap());
    let alternate_lba = u64::from_le_bytes(hdr[32..40].try_into().unwrap());
    let first_usable_lba = u64::from_le_bytes(hdr[40..48].try_into().unwrap());
    let last_usable_lba = u64::from_le_bytes(hdr[48..56].try_into().unwrap());
    let part_entry_lba = u64::from_le_bytes(hdr[72..80].try_into().unwrap());
    let num_entries = u32::from_le_bytes(hdr[80..84].try_into().unwrap());
    let entry_size = u32::from_le_bytes(hdr[84..88].try_into().unwrap()) as usize;

    if my_lba != 1 || alternate_lba == 0 || header_size < 92 || entry_size == 0 || num_entries == 0
    {
        trace!(
            my_lba,
            alternate_lba, header_size, "GPT header looks invalid"
        );
        return Ok(None);
    }

    dest.seek(SeekFrom::Start(part_entry_lba * 512))?;
    let entries_size = num_entries as usize * entry_size;
    let mut entries_raw = vec![0u8; entries_size];
    dest.read_exact(&mut entries_raw)?;

    let entries: Vec<PartitionEntry> = (0..num_entries as usize)
        .map(|i| {
            parse_entry(
                (i + 1) as u32,
                &entries_raw[i * entry_size..(i + 1) * entry_size],
            )
        })
        .filter(|e| e.is_used())
        .collect();

    let header = GptHeader {
        header_size,
        my_lba,
        alternate_lba,
        first_usable_lba,
        last_usable_lba,
        part_entry_lba,
        num_entries,
        entry_size,
        raw_sector: hdr,
    };

    Ok(Some(Gpt {
        header,
        entries,
        entries_raw,
    }))
}

/// Write the partition entries blob at `entries_lba` and update the GPT header
/// at `header_lba` so its PartitionEntriesCRC32 and HeaderCRC32 fields reflect
/// `entries_raw`. Other header fields are left as-is in `hdr_sector` and the
/// caller is responsible for setting them before calling this.
fn write_gpt_table<D: Write + Seek>(
    dest: &mut D,
    header_lba: u64,
    entries_lba: u64,
    hdr_sector: &mut [u8; 512],
    header_size: usize,
    entries_raw: &[u8],
) -> Result<()> {
    // Update the entries CRC and zero the header CRC before recomputing it.
    let entries_crc = crc32(entries_raw);
    hdr_sector[88..92].copy_from_slice(&entries_crc.to_le_bytes());
    hdr_sector[16..20].copy_from_slice(&[0u8; 4]);
    let header_crc = crc32(&hdr_sector[..header_size]);
    hdr_sector[16..20].copy_from_slice(&header_crc.to_le_bytes());

    dest.seek(SeekFrom::Start(entries_lba * 512))?;
    dest.write_all(entries_raw)?;
    dest.seek(SeekFrom::Start(header_lba * 512))?;
    dest.write_all(hdr_sector)?;
    Ok(())
}

/// Serialize a partition entry into its on-disk form (inverse of `parse_entry`).
/// The name is truncated to 36 UTF-16 code units.
fn entry_to_disk(entry: &PartitionEntry, entry_size: usize) -> Vec<u8> {
    let mut raw = vec![0u8; entry_size];
    raw[0..16].copy_from_slice(&guid_to_disk(&entry.type_guid));
    raw[16..32].copy_from_slice(&guid_to_disk(&entry.unique_guid));
    raw[32..40].copy_from_slice(&entry.first_lba.to_le_bytes());
    raw[40..48].copy_from_slice(&entry.last_lba.to_le_bytes());
    raw[48..56].copy_from_slice(&entry.attributes.to_le_bytes());
    for (i, c) in entry.name.encode_utf16().take(36).enumerate() {
        raw[56 + i * 2..58 + i * 2].copy_from_slice(&c.to_le_bytes());
    }
    raw
}

/// Write a complete GPT onto a blank disk of `disk_size` bytes: protective MBR
/// at LBA 0, primary header + entries at LBA 1-33, and backup entries + header
/// at the end of the disk. A random disk GUID is generated.
///
/// `entries` occupy consecutive slots starting at partition 1; their `index`
/// fields are ignored.
pub fn write_gpt<D: Write + Seek>(
    dest: &mut D,
    disk_size: u64,
    entries: &[PartitionEntry],
) -> Result<()> {
    const NUM_ENTRIES: u32 = 128;
    const ENTRY_SIZE: usize = 128;
    const HEADER_SIZE: usize = 92;

    let disk_last_lba = disk_size / 512 - 1;
    let entry_sectors = NUM_ENTRIES as u64 * ENTRY_SIZE as u64 / 512;
    let first_usable_lba = 2 + entry_sectors;
    let last_usable_lba = disk_last_lba - 1 - entry_sectors;
    let backup_entries_lba = disk_last_lba - entry_sectors;

    if entries.len() > NUM_ENTRIES as usize {
        anyhow::bail!("Too many partitions: {}", entries.len());
    }
    for entry in entries {
        if entry.first_lba < first_usable_lba || entry.last_lba > last_usable_lba {
            anyhow::bail!(
                "Partition '{}' ({}-{}) outside usable range {}-{}",
                entry.name,
                entry.first_lba,
                entry.last_lba,
                first_usable_lba,
                last_usable_lba
            );
        }
    }

    let mut entries_raw = vec![0u8; NUM_ENTRIES as usize * ENTRY_SIZE];
    for (i, entry) in entries.iter().enumerate() {
        entries_raw[i * ENTRY_SIZE..(i + 1) * ENTRY_SIZE]
            .copy_from_slice(&entry_to_disk(entry, ENTRY_SIZE));
    }

    // Protective MBR: one 0xEE entry covering LBA 1 to the end of the disk.
    let mut mbr = [0u8; 512];
    mbr[446 + 1..446 + 4].copy_from_slice(&[0x00, 0x02, 0x00]);
    mbr[446 + 4] = 0xEE;
    mbr[446 + 5..446 + 8].copy_from_slice(&[0xFF, 0xFF, 0xFF]);
    mbr[446 + 8..446 + 12].copy_from_slice(&1u32.to_le_bytes());
    mbr[446 + 12..446 + 16]
        .copy_from_slice(&(disk_last_lba.min(0xFFFF_FFFF) as u32).to_le_bytes());
    mbr[510] = 0x55;
    mbr[511] = 0xAA;
    dest.seek(SeekFrom::Start(0))?;
    dest.write_all(&mbr)?;

    // Header template shared by primary and backup.
    let mut hdr = [0u8; 512];
    hdr[0..8].copy_from_slice(b"EFI PART");
    hdr[8..12].copy_from_slice(&[0x00, 0x00, 0x01, 0x00]);
    hdr[12..16].copy_from_slice(&(HEADER_SIZE as u32).to_le_bytes());
    hdr[40..48].copy_from_slice(&first_usable_lba.to_le_bytes());
    hdr[48..56].copy_from_slice(&last_usable_lba.to_le_bytes());
    hdr[56..72].copy_from_slice(&guid_to_disk(&Uuid::new_v4()));
    hdr[80..84].copy_from_slice(&NUM_ENTRIES.to_le_bytes());
    hdr[84..88].copy_from_slice(&(ENTRY_SIZE as u32).to_le_bytes());

    let mut primary = hdr;
    primary[24..32].copy_from_slice(&1u64.to_le_bytes());
    primary[32..40].copy_from_slice(&disk_last_lba.to_le_bytes());
    primary[72..80].copy_from_slice(&2u64.to_le_bytes());
    write_gpt_table(dest, 1, 2, &mut primary, HEADER_SIZE, &entries_raw)?;

    let mut backup = hdr;
    backup[24..32].copy_from_slice(&disk_last_lba.to_le_bytes());
    backup[32..40].copy_from_slice(&1u64.to_le_bytes());
    backup[72..80].copy_from_slice(&backup_entries_lba.to_le_bytes());
    write_gpt_table(
        dest,
        disk_last_lba,
        backup_entries_lba,
        &mut backup,
        HEADER_SIZE,
        &entries_raw,
    )?;

    debug!(disk_size, partitions = entries.len(), "Wrote fresh GPT");
    Ok(())
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
    let gpt = match read_gpt(dest)? {
        Some(g) => g,
        None => {
            trace!("Skipping backup GPT fixup");
            return Ok(());
        }
    };

    let disk_last_lba = dest_size / 512 - 1;
    let backup_entries_lba = disk_last_lba - 32;
    // LastUsableLBA = disk_last_lba - 33 (backup header + 32 entry sectors)
    let last_usable_lba = disk_last_lba - 33;

    // Fix the Protective MBR at LBA 0 so partition-table tools that read the
    // PMBR (fdisk, parted) don't print a size-mismatch warning. The PMBR
    // contains one MBR entry (offset 446) with type 0xEE covering LBAs 1 to
    // disk_last_lba; only the sector count field (entry offset +12, 4 bytes,
    // LE) needs updating to match the actual disk size.
    fix_protective_mbr(dest, disk_last_lba)?;

    // Update primary header to reflect actual disk geometry.
    let mut primary_sector = gpt.header.raw_sector;
    primary_sector[32..40].copy_from_slice(&disk_last_lba.to_le_bytes());
    primary_sector[48..56].copy_from_slice(&last_usable_lba.to_le_bytes());
    write_gpt_table(
        dest,
        gpt.header.my_lba,
        gpt.header.part_entry_lba,
        &mut primary_sector,
        gpt.header.header_size,
        &gpt.entries_raw,
    )?;

    // Build the backup header by mutating a copy of the primary.
    let mut backup_sector = gpt.header.raw_sector;
    // Swap MyLBA and AlternateLBA
    backup_sector[24..32].copy_from_slice(&disk_last_lba.to_le_bytes());
    backup_sector[32..40].copy_from_slice(&gpt.header.my_lba.to_le_bytes());
    // LastUsableLBA
    backup_sector[48..56].copy_from_slice(&last_usable_lba.to_le_bytes());
    // PartitionEntryLBA → backup entries location
    backup_sector[72..80].copy_from_slice(&backup_entries_lba.to_le_bytes());
    write_gpt_table(
        dest,
        disk_last_lba,
        backup_entries_lba,
        &mut backup_sector,
        gpt.header.header_size,
        &gpt.entries_raw,
    )?;

    debug!(disk_last_lba, backup_entries_lba, "Wrote backup GPT header");
    Ok(())
}

/// Describes a partition whose `last_lba` was extended by
/// [`extend_last_partition`].
#[derive(Debug, Clone)]
pub struct ExtendedPartition {
    pub index: u32,
    pub first_lba: u64,
    pub new_last_lba: u64,
    pub type_guid: Uuid,
    pub unique_guid: Uuid,
}

/// Grow the trailing partition (the one with the highest `first_lba`) so its
/// `last_lba` reaches `last_usable_lba`. Updates the primary GPT in-place;
/// caller should follow up with [`fixup_backup_gpt`] to mirror to the backup.
///
/// Returns `Ok(None)` when there is no GPT, no used partitions, or the
/// trailing partition is already within 1 MiB of `last_usable_lba` (no
/// meaningful growth available).
pub fn extend_last_partition(dest: &mut File, dest_size: u64) -> Result<Option<ExtendedPartition>> {
    let gpt = match read_gpt(dest)? {
        Some(g) => g,
        None => return Ok(None),
    };

    let disk_last_lba = dest_size / 512 - 1;
    let target_last_lba = disk_last_lba - 33;

    let Some(target) = gpt.entries.iter().max_by_key(|e| e.first_lba).cloned() else {
        return Ok(None);
    };

    // Skip if the partition is already within ~1 MiB (2048 sectors) of the end.
    if target.last_lba + 2048 >= target_last_lba {
        info!(
            partition = target.index,
            current_last_lba = target.last_lba,
            target_last_lba,
            "Skipping partition extension: already at end of disk"
        );
        return Ok(None);
    }

    // Mutate the entry inside entries_raw.
    let mut entries_raw = gpt.entries_raw.clone();
    let entry_offset = (target.index as usize - 1) * gpt.header.entry_size;
    entries_raw[entry_offset + 40..entry_offset + 48]
        .copy_from_slice(&target_last_lba.to_le_bytes());

    let mut primary_sector = gpt.header.raw_sector;
    write_gpt_table(
        dest,
        gpt.header.my_lba,
        gpt.header.part_entry_lba,
        &mut primary_sector,
        gpt.header.header_size,
        &entries_raw,
    )?;

    info!(
        partition = target.index,
        first_lba = target.first_lba,
        new_last_lba = target_last_lba,
        "Extended trailing partition to end of disk"
    );

    Ok(Some(ExtendedPartition {
        index: target.index,
        first_lba: target.first_lba,
        new_last_lba: target_last_lba,
        type_guid: target.type_guid,
        unique_guid: target.unique_guid,
    }))
}

/// Update the Protective MBR's sector-count field at LBA 0 to reflect the
/// actual disk size. Cosmetic only — the kernel uses the GPT directly — but
/// silences `fdisk` / `parted` warnings like
/// "GPT PMBR size mismatch (X != Y) will be corrected by write".
fn fix_protective_mbr(dest: &mut File, disk_last_lba: u64) -> Result<()> {
    dest.seek(SeekFrom::Start(0))?;
    let mut mbr = [0u8; 512];
    dest.read_exact(&mut mbr)?;
    // Require boot signature and a protective-type entry at the first slot.
    if mbr[510] != 0x55 || mbr[511] != 0xAA || mbr[446 + 4] != 0xEE {
        trace!("LBA 0 is not a recognizable Protective MBR; leaving it alone");
        return Ok(());
    }
    // Sector count is capped at 0xFFFFFFFF for disks larger than 2 TiB.
    let count = disk_last_lba.min(0xFFFF_FFFF) as u32;
    mbr[446 + 12..446 + 16].copy_from_slice(&count.to_le_bytes());
    dest.seek(SeekFrom::Start(0))?;
    dest.write_all(&mbr)?;
    Ok(())
}

/// Open `path`, parse its GPT, and return the first partition entry whose
/// type GUID is the EFI System Partition GUID.
///
/// Multiboot (alloy) disks carry several ESPs and rely on the goldboot ESP
/// being partition 1, so "first ESP" resolves to the chain-loader.
pub fn find_esp(path: &Path) -> Result<Option<PartitionEntry>> {
    let mut f = OpenOptions::new().read(true).open(path)?;
    let Some(gpt) = read_gpt(&mut f)? else {
        return Ok(None);
    };
    Ok(gpt
        .entries
        .into_iter()
        .find(|e| e.type_guid == ESP_TYPE_GUID))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn test_entries() -> Vec<PartitionEntry> {
        vec![
            PartitionEntry {
                index: 1,
                type_guid: ESP_TYPE_GUID,
                unique_guid: Uuid::new_v4(),
                first_lba: 2048,
                last_lba: 4095,
                attributes: 0,
                name: "EFI system partition".into(),
            },
            PartitionEntry {
                index: 2,
                type_guid: Uuid::from_u128(0x0FC63DAF_8483_4772_8E79_3D69D8477DE4),
                unique_guid: Uuid::new_v4(),
                first_lba: 4096,
                last_lba: 8191,
                attributes: 1 << 62,
                name: "root".into(),
            },
        ]
    }

    #[test]
    fn write_gpt_read_gpt_round_trip() -> Result<()> {
        let disk_size: u64 = 16 * 1024 * 1024;
        let mut disk = Cursor::new(vec![0u8; disk_size as usize]);
        let entries = test_entries();
        write_gpt(&mut disk, disk_size, &entries)?;

        let gpt = read_gpt(&mut disk)?.expect("GPT not found after write");
        assert_eq!(gpt.entries.len(), entries.len());
        for (parsed, original) in gpt.entries.iter().zip(entries.iter()) {
            assert_eq!(parsed.type_guid, original.type_guid);
            assert_eq!(parsed.unique_guid, original.unique_guid);
            assert_eq!(parsed.first_lba, original.first_lba);
            assert_eq!(parsed.last_lba, original.last_lba);
            assert_eq!(parsed.attributes, original.attributes);
            assert_eq!(parsed.name, original.name);
        }

        let disk_last_lba = disk_size / 512 - 1;
        assert_eq!(gpt.header.my_lba, 1);
        assert_eq!(gpt.header.alternate_lba, disk_last_lba);
        assert_eq!(gpt.header.first_usable_lba, 34);
        assert_eq!(gpt.header.last_usable_lba, disk_last_lba - 33);
        Ok(())
    }

    #[test]
    fn write_gpt_backup_matches_primary_entries() -> Result<()> {
        let disk_size: u64 = 16 * 1024 * 1024;
        let mut disk = Cursor::new(vec![0u8; disk_size as usize]);
        write_gpt(&mut disk, disk_size, &test_entries())?;

        let disk_last_lba = disk_size / 512 - 1;
        let buf = disk.into_inner();
        let primary_entries = &buf[2 * 512..(2 + 32) * 512];
        let backup_entries =
            &buf[(disk_last_lba - 32) as usize * 512..disk_last_lba as usize * 512];
        assert_eq!(primary_entries, backup_entries);

        // Backup header signature present at the last LBA
        assert_eq!(
            &buf[disk_last_lba as usize * 512..disk_last_lba as usize * 512 + 8],
            b"EFI PART"
        );
        Ok(())
    }

    #[test]
    fn write_gpt_rejects_out_of_range_partition() {
        let disk_size: u64 = 16 * 1024 * 1024;
        let mut disk = Cursor::new(vec![0u8; disk_size as usize]);
        let mut entries = test_entries();
        entries[1].last_lba = disk_size / 512; // beyond the disk
        assert!(write_gpt(&mut disk, disk_size, &entries).is_err());
    }
}
