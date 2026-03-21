use super::levels::{L1Entry, L2Entry};
use binrw::{BinRead, binread, io::SeekFrom};
use std::io::{Read, Seek};

/// An entry in the snapshot table representing the system state at a moment in
/// time
#[binread]
#[derive(Debug)]
pub struct Snapshot {
    /// Offset into the image file at which the L1 table for the
    /// snapshot starts. Must be aligned to a cluster boundary.
    pub l1_table_offset: u64,

    /// Number of entries in the L1 table of the snapshot
    pub l1_entry_count: u32,

    /// Length of the unique ID string describing the snapshot
    #[br(temp)]
    unique_id_len: u16,

    /// Length of the name of the snapshot
    #[br(temp)]
    name_len: u16,

    /// Time at which the snapshot was taken since the Epoch
    pub time: SnapshotTime,

    /// Time that the guest was running until the snapshot was taken in
    /// nanoseconds
    pub guest_runtime: u64,

    /// Size of the VM state in bytes. 0 if no VM state is saved.
    ///
    /// If there is VM state, it starts at the first cluster
    /// described by first L1 table entry that doesn't describe a
    /// regular guest cluster (i.e. VM state is stored like guest
    /// disk content, except that it is stored at offsets that are
    /// larger than the virtual disk presented to the guest)
    pub vm_state_size: u32,

    #[br(temp)]
    extra_data_size: u32,

    /// Optional extra snapshot data that comes from format updates
    #[br(pad_size_to = extra_data_size)]
    #[br(args(extra_data_size))]
    pub extra_data: SnapshotExtraData,

    /// A unique identifier for the snapshot (example value: "1")
    #[br(count = unique_id_len, try_map = String::from_utf8)]
    pub unique_id: String,

    /// Name of the snapshot
    #[br(count = name_len, try_map = String::from_utf8)]
    pub name: String,

    /// Padding to align the entry to an 8-byte boundary.
    ///
    /// Fixed fields are 40 bytes (8+4+2+2+8+8+4+4), then extra_data_size,
    /// unique_id_len, and name_len bytes of variable data.
    #[br(count = (8 - ((40u64 + extra_data_size as u64 + unique_id_len as u64 + name_len as u64) % 8)) % 8)]
    _padding: Vec<u8>,
}

/// Optional extra snapshot data that comes from format updates
///
/// **Note:** Version 3 snapshots must have both vm_state_size and
/// virtual_disk_size present.
#[derive(BinRead, Debug)]
#[br(import(size: u32))]
pub struct SnapshotExtraData {
    /// Size of the VM state in bytes. 0 if no VM state is saved. If this field
    /// is present, the 32-bit value in Snapshot.vm_state_size is ignored.
    #[br(if(size >= 8))]
    pub vm_state_size: u64,

    /// Virtual disk size of the snapshot in bytes
    #[br(if(size >= 16))]
    pub virtual_disk_size: Option<u64>,

    /// icount value which corresponds to the record/replay instruction count
    /// when the snapshot was taken. Set to -1 if icount was disabled
    #[br(if(size >= 24))]
    pub instruction_count: Option<i64>,
}

/// Represents the time a snapshot was taken in the form of seconds,
/// nanoseconds. The nanoseconds represent the sub-second time of the snapshot.
#[derive(BinRead, Debug)]
pub struct SnapshotTime {
    /// Seconds since the unix epoch
    pub secs: u32,

    /// Subsecond portion of time in nanoseconds
    pub nanosecs: u32,
}

impl Snapshot {
    fn read_l1(&self, reader: &mut (impl Read + Seek)) -> Option<Vec<L1Entry>> {
        use binrw::BinReaderExt;
        reader
            .seek(SeekFrom::Start(self.l1_table_offset))
            .ok()?;
        (0..self.l1_entry_count)
            .map(|_| reader.read_be::<L1Entry>().ok())
            .collect()
    }

    /// Count the number of allocated clusters in this snapshot.
    pub fn count_clusters(
        &self,
        reader: &mut (impl Read + Seek),
        cluster_bits: u32,
    ) -> u64 {
        let mut count = 0;
        if let Some(l1_table) = self.read_l1(reader) {
            for l1_entry in &l1_table {
                if let Some(l2_table) = l1_entry.read_l2(reader, cluster_bits) {
                    for l2_entry in l2_table {
                        if l2_entry.is_allocated() {
                            count += 1;
                        }
                    }
                }
            }
        }
        count
    }

    /// Return all allocated clusters in this snapshot in virtual-disk order.
    ///
    /// Each item is `(block_offset, L2Entry)` where `block_offset` is the byte
    /// offset within the virtual disk that the cluster corresponds to.
    pub fn read_clusters(
        &self,
        reader: &mut (impl Read + Seek),
        cluster_bits: u32,
    ) -> Vec<(u64, L2Entry)> {
        let cluster_size = 1u64 << cluster_bits;
        let l2_entries_per_cluster = cluster_size / 8;
        let mut result = Vec::new();

        if let Some(l1_table) = self.read_l1(reader) {
            for (l1_index, l1_entry) in l1_table.iter().enumerate() {
                if let Some(l2_table) = l1_entry.read_l2(reader, cluster_bits) {
                    for (l2_index, l2_entry) in l2_table.into_iter().enumerate() {
                        if l2_entry.is_allocated() {
                            let block_offset = (l1_index as u64 * l2_entries_per_cluster
                                + l2_index as u64)
                                * cluster_size;
                            result.push((block_offset, l2_entry));
                        }
                    }
                }
            }
        }

        result
    }
}
