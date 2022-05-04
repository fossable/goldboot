use sha2::Digest;
use binrw::{BinRead, BinReaderExt, BinWrite};
use serde::{Deserialize, Serialize};
use std::{error::Error, fs::File, path::Path};
use validator::Validate;
use sha2::{Sha256};

pub mod qcow;

/// Represents an immutable goldboot image on disk which has the following binary format:
///
/// +---------------+
/// | File magic    | 4 bytes
/// +---------------+
/// | Metadata size | 2 bytes
/// +---------------+
/// | JSON metadata |
/// +---------------+
/// | Digest table  |
/// +---------------+
/// | Cluster table |
/// +---------------+
///
/// The target data is divided into equal size sections called "blocks". Blocks that are
/// nonzero will have an associated "cluster" allocated in the image file. Clusters
/// are variable in size and ideally smaller than their associated blocks (due to compression).
/// If a block does not have an associated cluster, that block is zero.
///
/// Every cluster is tracked in the digest table which is ordered
pub struct GoldbootImage {
	/// The image header
	pub header: ImageHeader,

	/// The image metadata
	pub metadata: ImageMetadata,

	/// The read-only image file
	pub file: std::fs::File,

	/// The filesystem path to the image file
	pub path: std::path::PathBuf,
}

impl GoldbootImage {
	pub fn open(path: impl AsRef<Path>) -> Result<Self, Box<dyn Error>> {
		let path = path.as_ref();
		let mut file = File::open(path)?;

		// Read header
		let header: ImageHeader = file.read_be()?;

		Ok(Self {
			file,
			path: path.to_path_buf(),
			metadata: serde_json::from_slice(&header.metadata)?,
			header,
		})
	}

	pub fn convert(
		source: &qcow::Qcow3,
		destination: impl AsRef<Path>,
	) -> Result<Self, Box<dyn Error>> {
		let path = destination.as_ref();
		let mut file = File::create(path)?;

		// Write header
		let header = ImageHeader {
			metadata_length: 123,
			metadata: Vec::new(),
		};
		header.write_to(&mut file);

		let buffer = [0_u8; 12];

		for l1_entry in source.l1_table {
			if let Some(l2_table) = l1_entry.read_l2(&mut file, source.header.cluster_bits) {
				for l2_entry in l2_table {
					if l2_entry.is_used {
						l2_entry.read_contents(&mut file, &mut buffer)?;

						// Compute hash
						let digest = Sha256::new().chain_update(&buffer).finalize();

						DigestTableEntry {
							digest,
						};
					}
				}
			}
		}
		Ok()
	}
}

/// The cluster compression algorithm.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum ClusterCompressionType {
	/// Clusters will not be compressed
	None,

	/// Clusters will be compressed with Zstandard
	Zstd,
}

/// The cluster encryption algorithm.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum ClusterEncryptionType {
	/// Clusters will not be encrypted
	None,

	/// Clusters will be encrypted with AES after compression
	Aes,
}

#[derive(BinRead, BinWrite, Debug)]
#[br(magic = b"\xc0\x1d\xb0\x01")]
pub struct ImageHeader {
	pub metadata_length: u16,

	#[br(count = metadata_length)]
	pub metadata: Vec<u8>,
}

#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct ImageMetadata {
	/// The format version
	pub version: u16,

	/// The size in bytes of each disk block
	pub block_size: u16,

	/// The number of populated clusters in this image
	pub cluster_count: u16,

	pub compression_type: ClusterCompressionType,

	pub encryption_type: ClusterEncryptionType,
}

#[derive(BinRead, BinWrite, Debug)]
pub struct DigestTableEntry {
	/// The cluster's offset in the image file
	pub cluster_offset: u64,

	/// The block's offset in the real data
	pub block_offset: u64,

	/// The SHA256 hash of the block before compression and encryption
	pub digest: [u8; 32],
}

#[derive(BinRead, BinWrite, Debug)]
pub struct Cluster {
	/// The size of the cluster in bytes
	pub size: u32,

	/// The cluster data which might be compressed and encrypted
	#[br(count = size)]
	pub data: Vec<u8>,
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_load() -> Result<(), Box<dyn Error>> {
		Ok(())
	}
}
