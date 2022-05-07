use binrw::{BinRead, BinReaderExt, BinWrite};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
	error::Error,
	fs::File,
	io::{BufReader, Read, Seek, SeekFrom, Write},
	path::Path,
};
use validator::Validate;

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

		let metadata = ImageMetadata {
			version: 1,
			block_size: source.header.cluster_size() as u16,
			cluster_count: source.count_clusters()?,
			compression_type: ClusterCompressionType::Zstd,
			encryption_type: ClusterEncryptionType::None,
		};

		let metadata_bytes = serde_json::to_vec(&metadata)?;

		// Write header
		let header = ImageHeader {
			metadata_length: metadata_bytes.len() as u16,
			metadata: metadata_bytes,
		};
		header.write_to(&mut file);

		// Track the offset into the data
		let mut real_offset = 0;

		// Track the cluster offset in the image file
		let mut cluster_offset = 0;

		for l1_entry in &source.l1_table {
			if let Some(l2_table) = l1_entry.read_l2(&mut file, source.header.cluster_bits) {
				for l2_entry in l2_table {
					if l2_entry.is_used {
						let mut buffer = vec![0_u8; source.header.cluster_size() as usize];

						l2_entry.read_contents(&mut file, &mut buffer)?;

						// Compute hash
						let digest = Sha256::new().chain_update(&buffer).finalize();

						// Write new entry
						DigestTableEntry {
							digest: digest.into(),
							block_offset: real_offset,
							cluster_offset,
						}
						.write_to(&mut file)?;

						// Perform compression
						let buffer = match metadata.compression_type {
							ClusterCompressionType::None => buffer,
							ClusterCompressionType::Zstd => {
								zstd::encode_all(std::io::Cursor::new(buffer), 0)?
							}
						};

						// Perform encryption
						// TODO

						// Write the cluster
						file.seek(SeekFrom::Start(cluster_offset))?;
						let size: [u8; 4] =
							unsafe { std::mem::transmute((buffer.len() as u32).to_be()) };
						file.write_all(&size)?;
						file.write_all(&buffer)?;

						// Advance offset
						cluster_offset += 4;
						cluster_offset += buffer.len() as u64;
					}
					real_offset += source.header.cluster_size();
				}
			} else {
				real_offset +=
					source.header.cluster_size() * source.header.l2_entries_per_cluster();
			}
		}
		Ok(Self {
			path: path.to_path_buf(),
			metadata,
			header,
		})
	}

	/// Write the image out to disk.
	pub fn write<D: Read + Seek + Write>(&self, mut dest: D) -> Result<(), Box<dyn Error>> {
		// Open the digest table for reading
		let mut digest_table = BufReader::new(File::open(&self.path)?);
		digest_table.seek(SeekFrom::Start(6 + self.header.metadata_length as u64))?;

		// Open the cluster table for reading
		let mut cluster_table = BufReader::new(File::open(&self.path)?);
		cluster_table.seek(SeekFrom::Start(
			6 + self.header.metadata_length as u64 + self.metadata.cluster_count as u64 * 48,
		))?; // TODO magic numbers

		for _ in 0..self.metadata.cluster_count {
			// Read the digest table entry
			let entry: DigestTableEntry = digest_table.read_be()?;

			// Jump to the block corresponding to the cluster
			dest.seek(SeekFrom::Start(entry.block_offset))?;

			let mut block = vec![0u8; self.metadata.block_size as usize];
			dest.read_exact(&mut block)?;
			let hash: [u8; 32] = Sha256::new().chain_update(&block).finalize().into();

			if hash != entry.digest {
				// Read cluster
				let mut cluster: Cluster = cluster_table.read_be()?;

				// Reverse encryption
				// TODO

				// Reverse compression
				cluster.data = match self.metadata.compression_type {
					ClusterCompressionType::None => cluster.data,
					ClusterCompressionType::Zstd => {
						zstd::decode_all(std::io::Cursor::new(&cluster.data))?
					}
				};

				// Write the cluster to the block
				dest.write_all(&cluster.data)?;
			}
		}

		Ok(())
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
