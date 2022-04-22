use crate::*;

/// An entry in the snapshot table representing the system state at a moment in time
#[derive(BinRead, BinWrite, Debug)]
pub struct Snapshot {
	/// Offset into the image file at which the L1 table for the
	/// snapshot starts. Must be aligned to a cluster boundary.
	l1_table_offset: u64,

	/// Number of entries in the L1 table of the snapshots
	l1_entry_count: u32,

	/// Table of L1 entries in the screenshot
	#[br(restore_position, seek_before = SeekFrom::Start(l1_table_offset), count = l1_entry_count)]
	pub l1_table: Vec<L1Entry>,

	/// Length of the unique ID string describing the snapshot
	unique_id_len: u16,

	/// Length of the name of the snapshot
	name_len: u16,

	/// Time at which the snapshot was taken since the Epoch
	pub time: SnapshotTime,

	/// Time that the guest was running until the snapshot was taken in nanoseconds
	pub guest_runtime: u64,

	/// Size of the VM state in bytes. 0 if no VM state is saved.
	///
	/// If there is VM state, it starts at the first cluster
	/// described by first L1 table entry that doesn't describe a
	/// regular guest cluster (i.e. VM state is stored like guest
	/// disk content, except that it is stored at offsets that are
	/// larger than the virtual disk presented to the guest)
	pub vm_state_size: u32,

	extra_data_size: u32,

	/// Optional extra snapshot data that comes from format updates
	#[br(pad_size_to = extra_data_size)]
	#[br(args(extra_data_size))]
	pub extra_data: SnapshotExtraData,
	// A unique identifier for the snapshot (example value: "1")
	//#[br(count = unique_id_len, try_map = String::from_utf8)]
	//pub unique_id: String,

	// Name of the snapshot
	//#[br(count = name_len, try_map = String::from_utf8)]
	//pub name: String,
}

/// Optional extra snapshot data that comes from format updates
///
/// **Note:** Version 3 snapshots must have both vm_state_size and virtual_disk_size present.
#[derive(BinRead, BinWrite, Debug)]
#[br(import(size: u32))]
pub struct SnapshotExtraData {
	/// Size of the VM state in bytes. 0 if no VM state is saved. If this field is present,
	/// the 32-bit value in Snapshot.vm_state_size is ignored.
	#[br(if(size >= 8))]
	pub vm_state_size: u64,

	/// Virtual disk size of the snapshot in bytes
	#[br(if(size >= 16))]
	pub virtual_disk_size: Option<u64>,

	/// icount value which corresponds to the record/replay instruction count when the snapshot was
	/// taken. Set to -1 if icount was disabled
	#[br(if(size >= 24))]
	pub instruction_count: Option<i64>,
}

/// Represents the time a snapshot was taken in the form of seconds, nanoseconds. The nanoseconds
/// represent the sub-second time of the snapshot.
#[derive(BinRead, BinWrite, Debug)]
pub struct SnapshotTime {
	/// Seconds since the unix epoch
	pub secs: u32,

	/// Subsecond portion of time in nanoseconds
	pub nanosecs: u32,
}
