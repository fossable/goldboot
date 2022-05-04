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
	///
	/// **Note**: backing files are incompatible with raw external data
	/// files (auto-clear feature bit 1).
	backing_file_offset: u64,

	/// Length of the backing file name in bytes. Must not be
	/// longer than 1023 bytes. Undefined if the image doesn't have
	/// a backing file.
	backing_file_size: u32,

	/// Number of bits that are used for addressing an offset
	/// within a cluster (1 << cluster_bits is the cluster size).
	/// Must not be less than 9 (i.e. 512 byte clusters).
	///
	/// **Note**: qemu as of today has an implementation limit of 2 MB
	/// as the maximum cluster size and won't be able to open images
	/// with larger cluster sizes.
	///
	/// **Note**: if the image has Extended L2 Entries then cluster_bits
	/// must be at least 14 (i.e. 16384 byte clusters).
	pub cluster_bits: u32,

	/// Virtual disk size in bytes.
	///
	/// **Note**: qemu has an implementation limit of 32 MB as
	/// the maximum L1 table size.  With a 2 MB cluster
	/// size, it is unable to populate a virtual cluster
	/// beyond 2 EB (61 bits); with a 512 byte cluster
	/// size, it is unable to populate a virtual size
	/// larger than 128 GB (37 bits).  Meanwhile, L1/L2
	/// table layouts limit an image to no more than 64 PB
	/// (56 bits) of populated clusters, and an image may
	/// hit other limits first (such as a file system's
	/// maximum size).
	pub size: u64,

	/// Encryption method to use for contents
	crypt_method: u32,

	/// Number of entries in the active L1 table
	pub l1_size: u32,

	/// Offset into the image file at which the active L1 table
	/// starts. Must be aligned to a cluster boundary.
	pub l1_table_offset: u64,

	/// Offset into the image file at which the refcount table
	/// starts. Must be aligned to a cluster boundary.
	refcount_table_offset: u64,

	/// Number of clusters that the refcount table occupies
	refcount_table_clusters: u32,

	/// Number of snapshots contained in the image
	nb_snapshots: u32,

	/// Offset into the image file at which the snapshot table
	/// starts. Must be aligned to a cluster boundary.
	snapshots_offset: u64,

	/// Bitmask of incompatible features. An implementation must fail to open an image if an
	/// unknown bit is set.
	#[br(align_after = 8)]
	incompatible_features: u64,

	/// Bitmask of compatible features. An implementation can safely ignore any unknown bits
	/// that are set.
	compatible_features: u64,

	/// Bitmask of auto-clear features. An implementation may only write to an image with unknown
	/// auto-clear features if it clears the respective bits from this field first.
	autoclear_features: u64,

	/// Describes the width of a reference count block entry (width
	/// in bits: refcount_bits = 1 << refcount_order). For version 2
	/// images, the order is always assumed to be 4
	/// (i.e. refcount_bits = 16).
	/// This value may not exceed 6 (i.e. refcount_bits = 64).
	refcount_order: u32,

	header_len: u32,

	/// Defines the compression method used for compressed clusters.
	///
	/// All compressed clusters in an image use the same compression
	/// type.
	///
	/// If the incompatible bit "Compression type" is set: the field
	/// must be present and non-zero (which means non-zlib
	/// compression type). Otherwise, this field must not be present
	/// or must be zero (which means zlib).
	compression_type: u8,

	/// Marks the end of the extensions
	end: u32,
}

impl QcowHeader {
	/// Get the size of a cluster in bytes from the qcow
	pub fn cluster_size(&self) -> u64 {
		1 << self.cluster_bits
	}
}
