use crate::*;

/// Qcow header version 3 with metadata extensions.
#[derive(BinRead, BinWrite, Debug)]
#[brw(magic = b"QFI\xfb")]
pub struct QcowHeader {
    /// Version of the QCOW format.
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
    pub crypt_method: EncryptionMethod,

    /// Number of entries in the active L1 table
    pub l1_size: u32,

    /// Offset into the image file at which the active L1 table
    /// starts. Must be aligned to a cluster boundary.
    pub l1_table_offset: u64,

    /// Offset into the image file at which the refcount table
    /// starts. Must be aligned to a cluster boundary.
    pub refcount_table_offset: u64,

    /// Number of clusters that the refcount table occupies
    pub refcount_table_clusters: u32,

    /// Number of snapshots contained in the image
    pub(crate) nb_snapshots: u32,

    /// Offset into the image file at which the snapshot table
    /// starts. Must be aligned to a cluster boundary.
    pub(crate) snapshots_offset: u64,

    /// Bitmask of incompatible features. An implementation must fail to open an image if an
    /// unknown bit is set.
    #[brw(align_after = 8)]
    pub incompatible_features: u64,

    /// Bitmask of compatible features. An implementation can safely ignore any unknown bits
    /// that are set.
    pub compatible_features: u64,

    /// Bitmask of auto-clear features. An implementation may only write to an image with unknown
    /// auto-clear features if it clears the respective bits from this field first.
    pub autoclear_features: u64,

    /// Describes the width of a reference count block entry (width
    /// in bits: refcount_bits = 1 << refcount_order). For version 2
    /// images, the order is always assumed to be 4
    /// (i.e. refcount_bits = 16).
    /// This value may not exceed 6 (i.e. refcount_bits = 64).
    pub refcount_order: u32,

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
    pub compression_type: CompressionType,

    /// Metadata extension length
    #[brw(magic = 0x80818080_u32)]
    metadata_len: u32,

    /// The metadata itself
    #[br(count = metadata_len)]
    #[bw()]
    pub metadata: Vec<u8>,

    /// Marks the end of the extensions
    end: u32,
}

/// Encryption method (if any) to use for image contents.
#[derive(BinRead, BinWrite, Debug, Clone, Copy, PartialEq, Eq)]
#[brw(repr(u32))]
pub enum EncryptionMethod {
    /// No encryption is being used. This is the default.
    None = 0,

    /// Cluster contents are AES encrypted
    Aes = 1,

    /// Uses LUKS from drive encryption
    Luks = 2,
}

/// Compression type used for compressed clusters.
#[derive(BinRead, BinWrite, Debug, Clone, Copy, PartialEq, Eq)]
#[brw(repr(u8))]
pub enum CompressionType {
    /// Uses flate/zlib compression for any clusters which are compressed
    Zlib = 0,

    /// Uses zstandard compression for any clusters which are compressed
    Zstd = 1,
}

impl Default for CompressionType {
    fn default() -> Self {
        Self::Zlib
    }
}

impl QcowHeader {
    /// Get the size of a cluster in bytes from the qcow
    pub fn cluster_size(&self) -> u64 {
        1 << self.cluster_bits
    }

    pub fn new(size: u64, metadata: Vec<u8>) -> Self {
        let cluster_bits = 16;

        Self {
            version: 3,
            backing_file_offset: 0,
            backing_file_size: 0,
            cluster_bits,
            size,
            crypt_method: EncryptionMethod::None,
            // Algorithm taken from:
            // https://github.com/qemu/qemu/blob/04ddcda6a2387274b3f31a501be3affd172aea3d/block/qcow2.h#L678
            l1_size: ((size + (1 << (cluster_bits + (cluster_bits - 8_u32.trailing_zeros()))))
                >> (cluster_bits + (cluster_bits - 8_u32.trailing_zeros())))
                as u32,
            l1_table_offset: 196608,
            refcount_table_offset: 65536,
            refcount_table_clusters: 1,
            nb_snapshots: 0,
            snapshots_offset: 0,
            incompatible_features: 0,
            compatible_features: 0,
            autoclear_features: 0,
            refcount_order: 4,
            header_len: 112 + (4 + metadata.len()) as u32,
            compression_type: CompressionType::Zlib,
            metadata_len: metadata.len() as u32,
            metadata,
            end: 0_u32,
        }
    }
}
