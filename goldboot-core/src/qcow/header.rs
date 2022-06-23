use binrw::BinRead;

/// Qcow header version 3.
#[derive(BinRead, Debug)]
#[br(magic = b"QFI\xfb")]
pub struct QcowHeader {
	/// Version of the QCOW format.
	#[br(assert(version == 3))]
	pub version: u32,

	/// Offset into the image file at which the backing file name
	/// is stored (NB: The string is not null terminated). 0 if the
	/// image doesn't have a backing file.
	_backing_file_offset: u64,

	/// Length of the backing file name in bytes. Must not be
	/// longer than 1023 bytes. Undefined if the image doesn't have
	/// a backing file.
	_backing_file_size: u32,

	/// Number of bits that are used for addressing an offset
	/// within a cluster (1 << cluster_bits is the cluster size).
	/// Must not be less than 9 (i.e. 512 byte clusters).
	pub cluster_bits: u32,

	/// Virtual disk size in bytes.
	pub size: u64,

	/// Encryption method to use for contents
	_crypt_method: u32,

	/// Number of entries in the active L1 table
	pub l1_size: u32,

	/// Offset into the image file at which the active L1 table
	/// starts. Must be aligned to a cluster boundary.
	pub l1_table_offset: u64,

	/// Offset into the image file at which the refcount table
	/// starts. Must be aligned to a cluster boundary.
	_refcount_table_offset: u64,

	/// Number of clusters that the refcount table occupies
	_refcount_table_clusters: u32,

	/// Number of snapshots contained in the image
	_nb_snapshots: u32,

	/// Offset into the image file at which the snapshot table
	/// starts. Must be aligned to a cluster boundary.
	_snapshots_offset: u64,

	/// Bitmask of incompatible features. An implementation must fail to open an
	/// image if an unknown bit is set.
	#[br(align_after = 8)]
	_incompatible_features: u64,

	/// Bitmask of compatible features. An implementation can safely ignore any
	/// unknown bits that are set.
	_compatible_features: u64,

	/// Bitmask of auto-clear features. An implementation may only write to an
	/// image with unknown auto-clear features if it clears the respective bits
	/// from this field first.
	_autoclear_features: u64,

	/// Describes the width of a reference count block entry (width
	/// in bits: refcount_bits = 1 << refcount_order). For version 2
	/// images, the order is always assumed to be 4
	/// (i.e. refcount_bits = 16).
	/// This value may not exceed 6 (i.e. refcount_bits = 64).
	_refcount_order: u32,

	/// Total length of the header.
	pub header_len: u32,

	/// Defines the compression method used for compressed clusters.
	///
	/// All compressed clusters in an image use the same compression
	/// type.
	///
	/// If the incompatible bit "Compression type" is set: the field
	/// must be present and non-zero (which means non-zlib
	/// compression type). Otherwise, this field must not be present
	/// or must be zero (which means zlib).
	#[br(if(header_len > 104))]
	pub compression_type: CompressionType,

	/// Marks the end of the extensions
	_end: u32,
}

/// Compression type used for compressed clusters.
#[derive(BinRead, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[br(repr(u8))]
pub enum CompressionType {
	/// Uses flate/zlib compression for any clusters which are compressed
	#[default]
	Zlib = 0,

	/// Uses zstandard compression for any clusters which are compressed
	Zstd = 1,
}

impl QcowHeader {
	/// Get the size of a cluster in bytes from the qcow
	pub fn cluster_size(&self) -> u64 {
		1 << self.cluster_bits
	}

	/// Get the number of entries in an L2 table.
	pub fn l2_entries_per_cluster(&self) -> u64 {
		self.cluster_size() / 8
	}
}
