use crate::{progress::ProgressBar, qcow::Qcow3, BuildConfig};
use aes_gcm::{
	aead::{Aead, NewAead},
	Aes256Gcm, Key, Nonce,
};
use binrw::{BinRead, BinReaderExt, BinWrite};
use log::{debug, info, trace};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use simple_error::bail;
use std::{
	error::Error,
	fs::File,
	io::{BufReader, Cursor, Read, Seek, SeekFrom, Write},
	path::Path,
	time::{SystemTime, UNIX_EPOCH},
};
use validator::Validate;

/// Represents a goldboot image on disk.
///
/// # Binary format
///
/// | Section             | Size      | Encryption Key |
/// |---------------------|-----------|----------------|
/// | Primary Header      |           | None           |
/// | Protected Header    |           | Password       |
/// | Image Config        |           | Password       |
/// | Vault               |           | Password       |
/// | Digest Table        |           | Cluster Key    |
/// | Cluster Table       |           | Cluster Key    |
///
/// The target data is divided into equal size sections called "blocks". Blocks
/// that are nonzero will have an associated "cluster" allocated in the image
/// file. Clusters are variable in size and ideally smaller than their
/// associated blocks (due to compression). If a block does not have an
/// associated cluster, that block is zero.
pub struct ImageHandle {
	/// The primary file header
	pub primary_header: PrimaryHeader,

	/// The secondary header
	pub protected_header: ProtectedHeader,

	/// The config used to build the image
	pub config: BuildConfig,

	/// The filesystem path to the image file
	pub path: std::path::PathBuf,

	/// The image's ID (SHA256 hash)
	pub id: String,

	/// The size in bytes of the image file on disk
	pub file_size: u64,
}

/// The cluster compression algorithm.
#[derive(BinRead, BinWrite, Debug)]
#[brw(repr(u8))]
pub enum ClusterCompressionType {
	/// Clusters will not be compressed
	None = 0,

	/// Clusters will be compressed with Zstandard
	Zstd = 1,
}

/// The cluster encryption algorithm.
#[derive(BinRead, BinWrite, Debug)]
#[brw(repr(u8))]
pub enum ClusterEncryptionType {
	/// Clusters will not be encrypted
	None = 0,

	/// Clusters will be encrypted with AES256 GCM after compression
	Aes256 = 1,
}

/// The header encryption algorithm.
#[derive(BinRead, BinWrite, Debug)]
#[brw(repr(u8))]
pub enum HeaderEncryptionType {
	/// The header will not be encrypted
	None = 0,

	/// The header will be encrypted with AES256 GCM
	Aes256 = 1,
}

/// Contains metadata which is always plaintext. Anything potentially useful to
/// an attacker should instead reside in the protected header unless the user
/// may want to read it without decrypting the image first.
#[derive(BinRead, BinWrite, Debug)]
#[brw(magic = b"\xc0\x1d\xb0\x01", big)]
pub struct PrimaryHeader {
	/// The format version
	#[br(assert(version == 1))]
	pub version: u8,

	/// The total size of all blocks combined in bytes
	pub size: u64,

	/// Image creation time
	pub timestamp: u64,

	/// The encryption type for the protected header, config, and vault
	pub encryption_type: HeaderEncryptionType,

	/// A copy of the name field from the config
	pub name: [u8; 64],

	/// Protected header nonce
	pub protected_nonce: [u8; 12],
}

impl PrimaryHeader {
	pub fn size() -> u64 {
		// file magic
		4 +
		// version
		1 +
		// size
		8 +
		// timestamp
		8 +
		// encryption_type
		1 +
		// name
		64 +
		// protected_nonce
		12
	}
}

/// Contains metadata which may be encrypted. The security of these entries
/// isn't critical, but keeping them secret can impede ciphertext analysis.
#[derive(BinRead, BinWrite, Debug)]
#[brw(big)]
pub struct ProtectedHeader {
	/// The size in bytes of each disk block
	pub block_size: u32,

	/// The number of populated clusters in this image
	pub cluster_count: u32,

	/// The compression algorithm used on clusters
	pub compression_type: ClusterCompressionType,

	/// The encryption type for the digest table and all clusters
	pub encryption_type: ClusterEncryptionType,

	/// The nonce value used to encrypt the config
	pub config_nonce: [u8; 12],

	/// The size of the config in bytes
	pub config_size: u32,

	/// The nonce value used to encrypt the vault
	pub vault_nonce: [u8; 12],
}

impl ProtectedHeader {
	pub fn size() -> u64 {
		// block_size
		4 +
		// cluster_count
		4 +
		// compression_type
		1 +
		// encryption_type
		1 +
		// config_nonce
		12 +
		// config_size
		4 +
		// vault_nonce
		12
	}
}

/// Contains the cluster encryption key and nonces for each cluster. This is
/// what we actually need to protect.
#[derive(BinRead, BinWrite, Debug)]
#[brw(big)]
pub struct Vault {
	/// The encryption key for all clusters
	pub cluster_key: [u8; 128],

	/// The number of cluster nonces (and therefore the number of clusters)
	pub nonce_count: u32,

	/// A nonce for each cluster. These aren't actually sensitive
	#[br(count = nonce_count)]
	pub nonce_table: Vec<[u8; 12]>,
}

impl Vault {
	pub fn size(cluster_count: u32) -> u64 {
		// cluster_key
		128 +
		// nonce_count
		4 +
		// nonce_table
		12 * cluster_count as u64
	}
}

/// An entry in the digest table which corresponds to one cluster.
#[derive(BinRead, BinWrite, Debug)]
#[brw(big)]
pub struct DigestTableEntry {
	/// The cluster's offset in the image file
	pub cluster_offset: u64,

	/// The block's offset in the real data
	pub block_offset: u64,

	/// The SHA256 hash of the original block before compression and encryption
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

/// Represents a data cluster in the image file. Each cluster corresponds to a
/// fixed-size block in the user data.
#[derive(BinRead, BinWrite, Debug)]
#[brw(big)]
pub struct Cluster {
	/// The size of the cluster in bytes
	pub size: u32,

	/// The cluster data which might be compressed and encrypted
	#[br(count = size)]
	pub data: Vec<u8>,
}

impl ImageHandle {
	fn seek_primary_header<S: Seek>(&self, stream: &S) -> Result<(), Box<dyn Error>> {
		stream.seek(SeekFrom::Start(0))?;
		Ok(())
	}

	fn seek_protected_header<S: Seek>(&self, stream: &S) -> Result<(), Box<dyn Error>> {
		stream.seek(SeekFrom::Start(PrimaryHeader::size()))?;
		Ok(())
	}

	fn seek_config<S: Seek>(&self, stream: &S) -> Result<(), Box<dyn Error>> {
		stream.seek(SeekFrom::Start(
			PrimaryHeader::size() + ProtectedHeader::size(),
		))?;
		Ok(())
	}

	fn seek_vault<S: Seek>(&self, stream: &S) -> Result<(), Box<dyn Error>> {
		stream.seek(SeekFrom::Start(
			PrimaryHeader::size() + ProtectedHeader::size() + self.protected_header.config_size as u64,
		))?;
		Ok(())
	}

	fn seek_digest_table<S: Seek>(&self, stream: &S) -> Result<(), Box<dyn Error>> {
		stream.seek(SeekFrom::Start(
			PrimaryHeader::size()
				+ ProtectedHeader::size()
				+ self.protected_header.config_size as u64
				+ match self.protected_header.encryption_type {
					ClusterEncryptionType::None => 0,
					ClusterEncryptionType::Aes256 => {
						Vault::size(self.protected_header.cluster_count)
					}
				},
		))?;
		Ok(())
	}

	/// Read and decrypt the vault.
	fn read_vault(&self) -> Result<Vault, Box<dyn Error>> {
		match self.primary_header.encryption_type {
			HeaderEncryptionType::None => bail!("Encryption type is none"),
			HeaderEncryptionType::Aes256 => {
				let mut file = File::open(self.path)?;

				let mut vault_bytes = vec![0u8; self.protected_header.config_size as usize];
				self.seek_vault(&file)?;
				file.read_exact(&mut vault_bytes)?;

				let vault_bytes = cipher
					.decrypt(
						Nonce::from_slice(&self.protected_header.vault_nonce),
						vault_bytes.as_ref(),
					)
					.unwrap();
				Ok(Cursor::new(vault_bytes).read_be()?)
			}
		}
	}

	/// Read and decrypt the digest table.
	fn read_digest_table(&self) -> Result<Vec<DigestTableEntry>, Box<dyn Error>> {
		let mut file = File::open(self.path)?;

		let mut digest_table_bytes = vec![0u8; self.protected_header.config_size as usize];
		self.seek_digest_table(&file)?;
		file.read_exact(&mut digest_table_bytes)?;

		let cursor = match self.protected_header.encryption_type {
			ClusterEncryptionType::None => Cursor::new(digest_table_bytes),
			ClusterEncryptionType::Aes256 => {
				let digest_table_bytes = cipher
					.decrypt(
						Nonce::from_slice(&self.protected_header.vault_nonce),
						digest_table_bytes.as_ref(),
					)
					.unwrap();
				Cursor::new(digest_table_bytes)
			}
		};

		let mut digest_table = Vec::<DigestTableEntry>::new();
		for _ in 0..self.protected_header.cluster_count {
			digest_table.push(cursor.read_be()?);
		}

		Ok(digest_table)
	}

	/// Open a new handle on the given file.
	pub fn open(path: impl AsRef<Path>, password: Option<String>) -> Result<Self, Box<dyn Error>> {
		let path = path.as_ref();
		let mut file = File::open(path)?;

		debug!(
			"Opening image from: {}",
			&path.to_string_lossy().to_string()
		);

		let cipher = Aes256Gcm::new(Key::from_slice(
			password.unwrap_or("".to_string()).as_bytes(),
		));

		// Read primary header (always plaintext)
		let primary_header: PrimaryHeader = file.read_be()?;
		trace!("Read: {:?}", &primary_header);

		// Read protected header
		let protected_header: ProtectedHeader = match primary_header.encryption_type {
			HeaderEncryptionType::None => file.read_be()?,
			HeaderEncryptionType::Aes256 => {
				let mut protected_header_bytes = vec![0u8; ProtectedHeader::size() as usize];
				file.read_exact(&mut protected_header_bytes)?;

				let protected_header_bytes = cipher
					.decrypt(
						Nonce::from_slice(&primary_header.protected_nonce),
						protected_header_bytes.as_ref(),
					)
					.unwrap();
				Cursor::new(protected_header_bytes).read_be()?
			}
		};

		// Read config
		let config: BuildConfig = match primary_header.encryption_type {
			HeaderEncryptionType::None => {
				let mut config_bytes = vec![0u8; protected_header.config_size as usize];
				file.read_exact(&mut config_bytes)?;

				serde_json::from_slice(&config_bytes)?
			}
			HeaderEncryptionType::Aes256 => {
				let mut config_bytes = vec![0u8; protected_header.config_size as usize];
				file.read_exact(&mut config_bytes)?;

				let config_bytes = cipher
					.decrypt(
						Nonce::from_slice(&protected_header.config_nonce),
						config_bytes.as_ref(),
					)
					.unwrap();
				serde_json::from_slice(&config_bytes)?
			}
		};

		Ok(Self {
			primary_header,
			protected_header,
			config,
			path: path.to_path_buf(),
			file_size: std::fs::metadata(&path)?.len(),
			id: path.file_stem().unwrap().to_str().unwrap().to_string(),
		})
	}

	/// Modify the password that encrypts the encyption header. This doesn't
	/// re-encrypt the clusters because they are encrypted with the cluster key.
	pub fn change_password(&self, new_password: String) -> Result<(), Box<dyn Error>> {
		// Create the cipher and a RNG for the nonces
		let cipher = Aes256Gcm::new(Key::from_slice(new_password.as_bytes()));
		let mut rng = rand::thread_rng();

		// Generate nonces for all sections
		let protected_header_nonce = rng.gen::<[u8; 12]>();
		let config_nonce = rng.gen::<[u8; 12]>();
		let vault_nonce = rng.gen::<[u8; 12]>();

		// Reencrypt protected header
		let mut protected_header_bytes = Vec::new();
		self.protected_header.write(&mut protected_header_bytes)?;
		let protected_header_bytes = cipher
			.encrypt(
				Nonce::from_slice(&protected_header_nonce),
				protected_header_bytes.as_ref(),
			)
			.unwrap();

		// Reencrypt config
		let mut config_bytes = cipher
			.encrypt(
				Nonce::from_slice(&config_nonce),
				serde_json::to_vec(&self.config)?,
			)
			.unwrap();

		// Reencrypt vault
		let mut vault_bytes = Vec::new();
		self.read_vault().unwrap().write(&mut vault_bytes)?;
		let vault_bytes = cipher
			.encrypt(Nonce::from_slice(&vault_nonce), vault_bytes.as_ref())
			.unwrap();

		// Lastly modify the image file
		let mut file = File::open(self.path)?;

		ProtectedHeader::seek(&file)?;
		file.write_all(&protected_header_bytes)?;

		self.seek_config(&file)?;
		file.write_all(&config_bytes)?;

		self.seek_vault(&file)?;
		file.write_all(&vault_bytes)?;

		Ok(())
	}

	/// Convert a qcow image into a goldboot image.
	pub fn convert(
		source: &Qcow3,
		config: BuildConfig,
		dest: impl AsRef<Path>,
	) -> Result<ImageHandle, Box<dyn Error>> {
		info!("Exporting storage to goldboot image");

		let mut dest_file = File::create(&dest)?;
		let mut source_file = File::open(&source.path)?;

		// Prepare cipher and RNG if the image header should be encrypted
		let header_cipher = Aes256Gcm::new(Key::from_slice(
			config.password.clone().unwrap_or("".to_string()).as_bytes(),
		));
		let mut rng = rand::thread_rng();

		// Prepare primary header
		let primary_header = PrimaryHeader {
			version: 1,
			size: source.header.size,
			protected_nonce: rng.gen::<[u8; 12]>(),
			timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
			encryption_type: if config.password.is_some() {
				HeaderEncryptionType::Aes256
			} else {
				HeaderEncryptionType::None
			},
			name: config.name.clone().as_bytes(),
		};

		// Prepare config
		let mut config = config.clone();
		config.password = None;

		let config_bytes = serde_json::to_vec(&config)?;

		// Prepare protected header
		let protected_header = ProtectedHeader {
			block_size: source.header.cluster_size() as u32,
			cluster_count: source.count_clusters()? as u32,
			compression_type: ClusterCompressionType::Zstd,
			encryption_type: if config.password.is_some() {
				ClusterEncryptionType::Aes256
			} else {
				ClusterEncryptionType::None
			},
			config_nonce: rng.gen::<[u8; 12]>(),
			config_size: config_bytes.len() as u32,
			vault_nonce: rng.gen::<[u8; 12]>(),
		};

		// Prepare vault
		let vault = Vault {
			cluster_key: rng.gen::<[u8; 128]>(),
			nonce_count: protected_header.cluster_count,
			nonce_table: (0..protected_header.cluster_count)
				.map(|_| rng.gen::<[u8; 12]>())
				.collect(),
		};

		// Load the cluster cipher we just generated
		let cluster_cipher = Aes256Gcm::new(Key::from_slice(&vault.cluster_key));

		// Write primary header
		debug!("Writing: {:?}", &primary_header);
		primary_header.write_to(&mut dest_file)?;

		// Write protected header
		debug!("Writing: {:?}", &protected_header);
		match primary_header.encryption_type {
			HeaderEncryptionType::None => protected_header.write_to(&mut dest_file),
			HeaderEncryptionType::Aes256 => {
				let mut protected_header_bytes = Vec::new();
				protected_header.write(&mut protected_header_bytes)?;
				dest_file.write_all(
					&header_cipher
						.encrypt(
							Nonce::from_slice(&primary_header.protected_nonce),
							protected_header_bytes.as_ref(),
						)
						.unwrap(),
				)?;
			}
		}

		// Write config
		dest_file.write_all(&config_bytes)?;

		// Write vault
		match primary_header.encryption_type {
			HeaderEncryptionType::Aes256 => {
				let mut vault_bytes = Vec::new();
				vault.write_to(&mut vault_bytes)?;
				dest_file.write_all(
					&header_cipher
						.encrypt(
							Nonce::from_slice(&protected_header.vault_nonce),
							vault_bytes.as_ref(),
						)
						.unwrap(),
				)?;
			}
			_ => {}
		};

		// Track the offset into the data
		let mut block_offset = 0;

		// Track cluster ordinal so we can lookup cluster nonces later
		let mut cluster_count = 0;

		// Track the digest table offset in the image file
		let mut digest_entry_offset = PrimaryHeader::size()
			+ ProtectedHeader::size()
			+ protected_header.config_size as u64
			+ match protected_header.encryption_type {
				ClusterEncryptionType::None => 0,
				ClusterEncryptionType::Aes256 => Vault::size(protected_header.cluster_count),
			};

		// Track the cluster offset in the image file
		let mut cluster_offset =
			digest_entry_offset + DigestTableEntry::size() * protected_header.cluster_count as u64;

		// Setup progress bar
		let increment_progress = ProgressBar::Convert.new(source.header.size);

		// Read from the qcow2
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
						cluster.data = match protected_header.compression_type {
							ClusterCompressionType::None => cluster.data,
							ClusterCompressionType::Zstd => {
								zstd::encode_all(std::io::Cursor::new(cluster.data), 0)?
							}
						};

						// Perform encryption
						cluster.data = match protected_header.encryption_type {
							ClusterEncryptionType::None => cluster.data,
							ClusterEncryptionType::Aes256 => cluster_cipher
								.encrypt(
									Nonce::from_slice(&vault.nonce_table[cluster_count]),
									cluster.data.as_ref(),
								)
								.unwrap(),
						};

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
						cluster_count += 1;
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

		Ok(ImageHandle{
			primary_header,
			protected_header,
			config,
			path: dest.as_ref().to_path_buf(),
			id: todo!(),
			file_size: std::fs::metadata(&dest)?.len(),
})
	}

	/// TODO multi threaded WriteWorkers
	/// TODO write backup GPT header

	/// Write the image contents out to disk.
	pub fn write(
		&self,
		dest: impl AsRef<Path>,
		password: Option<String>,
	) -> Result<(), Box<dyn Error>> {
		info!("Writing image");

		let mut dest = std::fs::OpenOptions::new()
			.create(true)
			.write(true)
			.read(true)
			.open(dest)?;

		// Prepare cipher if the image should be decrypted
		let cipher = Aes256Gcm::new(Key::from_slice(
			password.unwrap_or("".to_string()).as_bytes(),
		));

		// Read the vault if we need it
		let vault = match self.protected_header.encryption_type {
			ClusterEncryptionType::None => todo!(),
			ClusterEncryptionType::Aes256 => self.read_vault()?,
		};

		let progress = ProgressBar::Write.new(
			self.protected_header.cluster_count as u64 * self.protected_header.block_size as u64,
		);

		// Read entire digest table. This may consume quite a bit of memory!
		let digest_table = self.read_digest_table()?;

		// Open the cluster table for reading
		let mut cluster_table = BufReader::new(File::open(&self.path)?);

		// Extend the file if necessary
		if dest.stream_len()? < self.primary_header.size {
			dest.set_len(self.primary_header.size)?;
		}

		let mut block = vec![0u8; self.protected_header.block_size as usize];

		// Write all of the clusters that have changed
		for i in 0..self.protected_header.cluster_count as usize {
			// Load digest table entry
			let entry = digest_table[i];

			// Jump to the block corresponding to the cluster
			dest.seek(SeekFrom::Start(entry.block_offset))?;
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
				cluster.data = match self.protected_header.encryption_type {
					ClusterEncryptionType::None => cluster.data,
					ClusterEncryptionType::Aes256 => cipher
						.decrypt(
							Nonce::from_slice(&vault.nonce_table[i]),
							cluster.data.as_ref(),
						)
						.unwrap(),
				};

				// Reverse compression
				cluster.data = match self.protected_header.compression_type {
					ClusterCompressionType::None => cluster.data,
					ClusterCompressionType::Zstd => {
						zstd::decode_all(std::io::Cursor::new(&cluster.data))?
					}
				};

				// Write the cluster to the block
				dest.seek(SeekFrom::Start(entry.block_offset))?;
				dest.write_all(&cluster.data)?;
			}

			progress(self.protected_header.block_size as u64);
		}

		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::Architecture;
	use sha1::Sha1;

	#[test]
	fn test_convert_empty() -> Result<(), Box<dyn Error>> {
		let tmp = tempfile::tempdir()?;

		GoldbootImage::convert(
			&Qcow3::open("test/empty.qcow2")?,
			BuildConfig {
				name: String::from("Empty test"),
				description: None,
				arch: Architecture::amd64,
				memory: None,
				nvme: None,
				password: None,
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
				arch: Architecture::amd64,
				memory: None,
				nvme: None,
				password: None,
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
