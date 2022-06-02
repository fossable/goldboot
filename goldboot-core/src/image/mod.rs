use crate::{progress::ProgressBar, qcow::Qcow3, BuildConfig};
use binrw::{BinRead, BinReaderExt, BinWrite};
use log::{debug, info, trace};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
	error::Error,
	fs::File,
	io::{BufReader, Read, Seek, SeekFrom, Write},
	path::Path,
	time::{SystemTime, UNIX_EPOCH},
};
use validator::Validate;

pub mod library;

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
/// Every cluster is tracked in the digest table which is ordered in the same order as the blocks.
pub struct GoldbootImage {
	/// The image header
	pub header: ImageHeader,

	/// The image metadata
	pub metadata: ImageMetadata,

	/// The filesystem path to the image file
	pub path: std::path::PathBuf,

	/// The image's ID (SHA256 hash)
	pub id: String,

	/// The size in bytes of the image file itself
	pub size: u64,
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
#[brw(magic = b"\xc0\x1d\xb0\x01", big)]
pub struct ImageHeader {
	pub metadata_length: u16,

	#[br(count = metadata_length)]
	pub metadata: Vec<u8>,
}

impl ImageHeader {
	pub fn size(&self) -> u64 {
		// file magic
		4 +
		// length field
		2 +
		// metadata
		self.metadata_length as u64
	}
}

#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct ImageMetadata {
	/// The format version
	pub version: u8,

	/// The total size of all blocks combined
	pub size: u64,

	/// The size in bytes of each disk block
	pub block_size: u64,

	/// The number of populated clusters in this image
	pub cluster_count: u64,

	/// Image creation time
	pub timestamp: u64,

	/// The config used to build the image
	pub config: BuildConfig,

	pub compression_type: ClusterCompressionType,

	pub encryption_type: ClusterEncryptionType,
}

#[derive(BinRead, BinWrite, Debug)]
#[brw(big)]
pub struct DigestTableEntry {
	/// The cluster's offset in the image file
	pub cluster_offset: u64,

	/// The block's offset in the real data
	pub block_offset: u64,

	/// The SHA256 hash of the block before compression and encryption
	pub digest: [u8; 32],
}

impl DigestTableEntry {
	pub fn size() -> u64 {
		// cluster_offset
		8 +
		// block_offset
		8 +
		// digest
		32
	}
}

#[derive(BinRead, BinWrite, Debug)]
#[brw(big)]
pub struct Cluster {
	/// The size of the cluster in bytes
	pub size: u32,

	/// The cluster data which might be compressed and encrypted
	#[br(count = size)]
	pub data: Vec<u8>,
}

impl GoldbootImage {
	pub fn open(path: impl AsRef<Path>) -> Result<Self, Box<dyn Error>> {
		let path = path.as_ref();
		let mut file = File::open(path)?;

		debug!(
			"Opening image from: {}",
			&path.to_string_lossy().to_string()
		);

		// Read header
		let header: ImageHeader = file.read_be()?;
		trace!("Read: {:?}", &header);

		Ok(Self {
			path: path.to_path_buf(),
			metadata: serde_json::from_slice(&header.metadata)?,
			header,
			size: std::fs::metadata(&path)?.len(),
			id: path.file_stem().unwrap().to_str().unwrap().to_string(),
		})
	}

	/// Convert a qcow image into a goldboot image.
	pub fn convert(
		source: &Qcow3,
		config: BuildConfig,
		destination: impl AsRef<Path>,
	) -> Result<(), Box<dyn Error>> {
		info!("Exporting storage to goldboot image");

		let mut dest_file = File::create(&destination)?;
		let mut source_file = File::open(&source.path)?;

		let metadata = ImageMetadata {
			version: 1,
			size: source.header.size,
			block_size: source.header.cluster_size(),
			cluster_count: source.count_clusters()?,
			timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
			config,
			compression_type: ClusterCompressionType::Zstd,
			encryption_type: ClusterEncryptionType::None,
		};

		let metadata_bytes = serde_json::to_vec(&metadata)?;

		// Write header
		let header = ImageHeader {
			metadata_length: metadata_bytes.len() as u16,
			metadata: metadata_bytes,
		};
		debug!("Writing: {:?}", &header);
		header.write_to(&mut dest_file)?;

		debug!("Writing: {:?}", &metadata);
		dest_file.write_all(&header.metadata)?;

		// Track the offset into the data
		let mut block_offset = 0;

		// Track the cluster offset in the image file
		let mut cluster_offset =
			header.size() + DigestTableEntry::size() * metadata.cluster_count as u64;

		// Tract the digest table offset in the image file
		let mut digest_entry_offset = header.size();

		// Setup progress bar
		let increment_progress = ProgressBar::Convert.new(source.header.size);

		for l1_entry in &source.l1_table {
			if let Some(l2_table) = l1_entry.read_l2(&mut source_file, source.header.cluster_bits) {
				for l2_entry in l2_table {
					if l2_entry.is_used {
						// Start building the cluster
						let mut cluster = Cluster {
							size: 0,
							data: vec![0_u8; source.header.cluster_size() as usize],
						};

						l2_entry.read_contents(
							&mut source_file,
							&mut cluster.data,
							source.header.compression_type,
						)?;

						// Compute hash
						let digest = Sha256::new().chain_update(&cluster.data).finalize();

						// Write new entry
						dest_file.seek(SeekFrom::Start(digest_entry_offset))?;
						let digest_entry = DigestTableEntry {
							digest: digest.into(),
							block_offset,
							cluster_offset,
						};

						digest_entry.write_to(&mut dest_file)?;
						digest_entry_offset += DigestTableEntry::size();

						// Perform compression
						cluster.data = match metadata.compression_type {
							ClusterCompressionType::None => cluster.data,
							ClusterCompressionType::Zstd => {
								zstd::encode_all(std::io::Cursor::new(cluster.data), 0)?
							}
						};

						// Perform encryption
						// TODO

						cluster.size = cluster.data.len() as u32;

						// Write the cluster
						trace!(
							"Writing {} byte cluster for: {:?}",
							cluster.size,
							&digest_entry
						);
						dest_file.seek(SeekFrom::Start(cluster_offset))?;
						cluster.write_to(&mut dest_file)?;

						// Advance offset
						cluster_offset += 4;
						cluster_offset += cluster.data.len() as u64;
					}
					block_offset += source.header.cluster_size();
					increment_progress(source.header.cluster_size());
				}
			} else {
				block_offset +=
					source.header.cluster_size() * source.header.l2_entries_per_cluster();
				increment_progress(
					source.header.cluster_size() * source.header.l2_entries_per_cluster(),
				);
			}
		}

		Ok(())
	}

	/// TODO multi threaded WriteWorkers

	/// Write the image out to disk.
	pub fn write(&self, dest: impl AsRef<Path>) -> Result<(), Box<dyn Error>> {
		info!("Writing image");

		let mut dest = std::fs::OpenOptions::new()
			.create(true)
			.write(true)
			.read(true)
			.open(dest)?;

		let progress = ProgressBar::Write
			.new(self.metadata.cluster_count as u64 * self.metadata.block_size as u64);

		// Open the digest table for reading
		let mut digest_table = BufReader::new(File::open(&self.path)?);
		digest_table.seek(SeekFrom::Start(self.header.size()))?;

		// Open the cluster table for reading
		let mut cluster_table = BufReader::new(File::open(&self.path)?);

		// Extend the file if necessary
		if dest.stream_len()? < self.metadata.size {
			dest.set_len(self.metadata.size)?;
		}

		// Write all of the clusters that have changed
		for _ in 0..self.metadata.cluster_count {
			// Read the digest table entry
			let entry: DigestTableEntry = digest_table.read_be()?;
			trace!("Read: {:?}", entry);

			// Jump to the block corresponding to the cluster
			dest.seek(SeekFrom::Start(entry.block_offset))?;

			let mut block = vec![0u8; self.metadata.block_size as usize];
			dest.read_exact(&mut block)?;

			let hash: [u8; 32] = Sha256::new().chain_update(&block).finalize().into();

			if hash != entry.digest {
				// Read cluster
				cluster_table.seek(SeekFrom::Start(entry.cluster_offset))?;
				let mut cluster: Cluster = cluster_table.read_be()?;

				trace!(
					"Read cluster of size {} from offset {}",
					cluster.size,
					entry.cluster_offset
				);

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
				dest.seek(SeekFrom::Start(entry.block_offset))?;
				dest.write_all(&cluster.data)?;
			}

			progress(self.metadata.block_size as u64);
		}

		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use sha1::Sha1;

	#[test]
	fn test_convert_empty() -> Result<(), Box<dyn Error>> {
		let tmp = tempfile::tempdir()?;

		GoldbootImage::convert(
			&Qcow3::open("test/empty.qcow2")?,
			BuildConfig {
				name: String::from("Empty test"),
				description: None,
				arch: None,
				memory: None,
				nvme: None,
				templates: vec![],
			},
			tmp.path().join("empty.gb"),
		)?;

		let image = GoldbootImage::open(tmp.path().join("empty.gb"))?;
		image.write(tmp.path().join("empty.raw"))?;

		Ok(())
	}

	#[test]
	fn test_convert_small() -> Result<(), Box<dyn Error>> {
		let tmp = tempfile::tempdir()?;

		GoldbootImage::convert(
			&Qcow3::open("test/small.qcow2")?,
			BuildConfig {
				name: String::from("Small test"),
				description: None,
				arch: None,
				memory: None,
				nvme: None,
				templates: vec![],
			},
			tmp.path().join("small.gb"),
		)?;

		let image = GoldbootImage::open(tmp.path().join("small.gb"))?;
		image.write(tmp.path().join("small.raw"))?;

		// Hash content
		assert_eq!(
			hex::encode(
				Sha1::new()
					.chain_update(&std::fs::read(tmp.path().join("small.raw"))?)
					.finalize()
			),
			"34e1c79c80941e5519ec76433790191318a5c77b"
		);

		Ok(())
	}
}
