//!

use crate::qcow::Qcow3;
use aes_gcm::KeyInit;
use aes_gcm::{aead::Aead, Aes256Gcm, Key, Nonce};
use anyhow::bail;
use anyhow::Result;
use binrw::{BinRead, BinReaderExt, BinWrite};
use rand::Rng;
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::ffi::CStr;
use std::{
    fs::File,
    io::{BufReader, Cursor, Read, Seek, SeekFrom, Write},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
use strum::{Display, EnumIter};
use tracing::{debug, info, trace};

pub mod qcow;

/// Supported system architectures for goldboot images.
#[derive(
    BinRead, BinWrite, Clone, Copy, Serialize, Deserialize, Debug, PartialEq, Eq, EnumIter, Display,
)]
#[serde(tag = "arch")]
#[brw(repr(u8))]
pub enum ImageArch {
    Amd64,
    Arm64,
    I386,
    Mips,
    Mips64,
    S390x,
}

impl Default for ImageArch {
    fn default() -> Self {
        match std::env::consts::ARCH {
            "x86" => ImageArch::I386,
            "x86_64" => ImageArch::Amd64,
            "aarch64" => ImageArch::Arm64,
            "mips" => ImageArch::Mips,
            "mips64" => ImageArch::Mips64,
            "s390x" => ImageArch::S390x,
            _ => panic!("Unknown CPU architecture: {}", std::env::consts::ARCH),
        }
    }
}

impl TryFrom<String> for ImageArch {
    type Error = anyhow::Error;
    fn try_from(s: String) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "amd64" => Ok(ImageArch::Amd64),
            "x86_64" => Ok(ImageArch::Amd64),
            "arm64" => Ok(ImageArch::Arm64),
            "aarch64" => Ok(ImageArch::Arm64),
            "i386" => Ok(ImageArch::I386),
            _ => bail!("Unknown architecture: {s}"),
        }
    }
}

/// Represents a goldboot image on disk.
///
/// # Binary format
///
/// | Section             | Encryption Key    |
/// |---------------------|-------------------|
/// | Primary Header      | None              |
/// | Protected Header    | Password + SHA256 |
/// | Image Config        | Password + SHA256 |
/// | Cluster Table       | Cluster Key       |
/// | Digest Table        | Password + SHA256 |
/// | Directory           | Password + SHA256 |
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
    pub protected_header: Option<ProtectedHeader>,

    /// The encoded config used to build the image
    pub config: Option<Vec<u8>>,

    /// The digest table
    pub digest_table: Option<DigestTable>,

    /// The section directory
    pub directory: Option<Directory>,

    /// The filesystem path to the image file
    pub path: std::path::PathBuf,

    /// The size in bytes of the image file on disk
    pub file_size: u64,

    /// The image's ID (SHA256 hash)
    pub id: String,
}

/// The cluster compression algorithm.
#[derive(BinRead, BinWrite, Debug, Eq, PartialEq, Clone)]
#[brw(repr(u8))]
pub enum ClusterCompressionType {
    /// Clusters will not be compressed
    None = 0,

    /// Clusters will be compressed with Zstandard
    Zstd = 1,
}

/// The cluster encryption algorithm.
#[derive(BinRead, BinWrite, Debug, Eq, PartialEq, Clone)]
#[brw(repr(u8))]
pub enum ClusterEncryptionType {
    /// Clusters will not be encrypted
    None = 0,

    /// Clusters will be encrypted with AES256 GCM after compression
    Aes256 = 1,
}

/// The header encryption algorithm.
#[derive(BinRead, BinWrite, Debug, Eq, PartialEq, Clone)]
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
#[derive(BinRead, BinWrite, Debug, Eq, PartialEq)]
#[brw(magic = b"\xc0\x1d\xb0\x01", big)]
pub struct PrimaryHeader {
    /// The format version
    #[br(assert(version == 1))]
    pub version: u8,

    /// The total size of all blocks combined in bytes
    pub size: u64,

    /// Image creation time
    pub timestamp: u64,

    /// The encryption type for metadata
    pub encryption_type: HeaderEncryptionType,

    /// A copy of the name field from the config
    pub name: [u8; 64],

    /// System architecture
    pub arch: ImageArch,

    /// Directory nonce
    pub directory_nonce: [u8; 12],

    /// The byte offset of the directory
    pub directory_offset: u64,

    /// The size of the directory in bytes
    pub directory_size: u32,
}

impl PrimaryHeader {
    pub fn name(&self) -> String {
        unsafe { CStr::from_ptr(self.name.as_ptr() as *const std::ffi::c_char) }
            .to_string_lossy()
            .into_owned()
    }
}

/// Contains metadata which may be encrypted.
#[derive(BinRead, BinWrite, Debug, Eq, PartialEq, Clone)]
#[brw(big)]
pub struct ProtectedHeader {
    /// The size in bytes of each disk block
    pub block_size: u32,

    /// The number of populated clusters in this image
    pub cluster_count: u32,

    /// The compression algorithm used on clusters
    pub cluster_compression: ClusterCompressionType,

    /// The encryption type for the digest table and all clusters
    pub cluster_encryption: ClusterEncryptionType,

    /// The number of cluster nonces if encryption is enabled
    pub nonce_count: u32,

    /// A nonce for each cluster if encryption is enabled
    #[br(count = nonce_count)]
    pub nonce_table: Vec<[u8; 12]>,

    /// The key for all clusters if encryption is enabled
    pub cluster_key: [u8; 32],
}

#[derive(BinRead, BinWrite, Debug)]
#[brw(big)]
pub struct Directory {
    /// Protected header nonce
    pub protected_nonce: [u8; 12],

    /// The size of the protected header in bytes
    pub protected_size: u32,

    /// The nonce value used to encrypt the config
    pub config_nonce: [u8; 12],

    /// The byte offset of the config
    pub config_offset: u64,

    /// The size of the config in bytes
    pub config_size: u32,

    /// The nonce value used to encrypt the digest table
    pub digest_table_nonce: [u8; 12],

    /// The byte offset of the digest table
    pub digest_table_offset: u64,

    /// The size of the digest table in bytes
    pub digest_table_size: u32,
}

#[derive(BinRead, BinWrite, Debug, Eq, PartialEq, Clone)]
#[brw(big)]
pub struct DigestTable {
    /// The number of digests (and therefore the number of clusters)
    pub digest_count: u32,

    /// A digest for each cluster
    #[br(count = digest_count)]
    pub digest_table: Vec<DigestTableEntry>,
}

/// An entry in the digest table which corresponds to one cluster.
#[derive(BinRead, BinWrite, Debug, Eq, PartialEq, Clone)]
#[brw(big)]
pub struct DigestTableEntry {
    /// The cluster's offset in the image file
    pub cluster_offset: u64,

    /// The block's offset in the real data
    pub block_offset: u64,

    /// The SHA256 hash of the original block before compression and encryption
    pub digest: [u8; 32],
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

/// Build an encryption key from the given password.
fn new_key(password: String) -> Aes256Gcm {
    // Hash so it's the correct length
    Aes256Gcm::new(&Sha256::new().chain_update(password.as_bytes()).finalize())
}

/// Hash the entire image file to produce the image ID.
pub fn compute_id(path: impl AsRef<Path>) -> Result<String> {
    let mut file = File::open(&path)?;
    let mut hasher = Sha256::new();

    std::io::copy(&mut file, &mut hasher)?;
    Ok(hex::encode(hasher.finalize()))
}

impl ImageHandle {
    /// Load all sections into memory except the cluster table. If the image is
    /// encrypted, the sections will be decrypted.
    pub fn load(&mut self, password: Option<String>) -> Result<()> {
        let mut file = File::open(&self.path)?;

        let cipher = new_key(password.unwrap_or("".to_string()));

        // Load the directory first because other sections rely on it
        file.seek(SeekFrom::Start(self.primary_header.directory_offset))?;
        let directory: Directory = match self.primary_header.encryption_type {
            HeaderEncryptionType::None => file.read_be()?,
            HeaderEncryptionType::Aes256 => {
                let mut directory_bytes = vec![0u8; self.primary_header.directory_size as usize];
                file.read_exact(&mut directory_bytes)?;

                let directory_bytes = cipher.decrypt(
                    Nonce::from_slice(&self.primary_header.directory_nonce),
                    directory_bytes.as_ref(),
                )?;
                Cursor::new(directory_bytes).read_be()?
            }
        };

        // Load the protected header
        file.seek(SeekFrom::Start(0))?;

        // Throw this away so we're at the correct offset
        let _primary: PrimaryHeader = file.read_be()?;
        let protected_header: ProtectedHeader = match self.primary_header.encryption_type {
            HeaderEncryptionType::None => file.read_be()?,
            HeaderEncryptionType::Aes256 => {
                let mut protected_header_bytes = vec![0u8; directory.protected_size as usize];
                file.read_exact(&mut protected_header_bytes)?;

                let protected_header_bytes = cipher.decrypt(
                    Nonce::from_slice(&directory.protected_nonce),
                    protected_header_bytes.as_ref(),
                )?;
                Cursor::new(protected_header_bytes).read_be()?
            }
        };

        // Load config
        file.seek(SeekFrom::Start(directory.config_offset))?;
        let mut config_bytes = vec![0u8; directory.config_size as usize];
        file.read_exact(&mut config_bytes)?;

        self.config = match self.primary_header.encryption_type {
            HeaderEncryptionType::None => Some(config_bytes),
            HeaderEncryptionType::Aes256 => Some(cipher.decrypt(
                Nonce::from_slice(&directory.config_nonce),
                config_bytes.as_ref(),
            )?),
        };

        // Load the digest table
        file.seek(SeekFrom::Start(directory.digest_table_offset))?;
        let digest_table: DigestTable = match self.primary_header.encryption_type {
            HeaderEncryptionType::None => file.read_be()?,
            HeaderEncryptionType::Aes256 => {
                let mut digest_table_bytes = vec![0u8; directory.digest_table_size as usize];
                file.read_exact(&mut digest_table_bytes)?;

                let digest_table_bytes = cipher.decrypt(
                    Nonce::from_slice(&directory.digest_table_nonce),
                    digest_table_bytes.as_ref(),
                )?;
                Cursor::new(digest_table_bytes).read_be()?
            }
        };

        // Modify the current image handle finally
        self.directory = Some(directory);
        self.protected_header = Some(protected_header);
        self.digest_table = Some(digest_table);
        Ok(())
    }

    /// Open a new handle on the given file.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let mut file = File::open(path)?;

        debug!("Opening image from: {}", path.display());

        // Read primary header (always plaintext)
        let primary_header: PrimaryHeader = file.read_be()?;
        trace!("Read: {:?}", &primary_header);

        // Get image ID
        let id = if let Some(stem) = path.file_stem() {
            if Regex::new("[A-Fa-f0-9]{64}")?.is_match(stem.to_str().unwrap()) {
                stem.to_str().unwrap().to_string()
            } else {
                compute_id(&path).unwrap()
            }
        } else {
            compute_id(&path).unwrap()
        };

        if primary_header.encryption_type == HeaderEncryptionType::None {
            // Read protected header
            let protected_header: ProtectedHeader = file.read_be()?;

            // Read directory
            file.seek(SeekFrom::Start(primary_header.directory_offset))?;
            let directory: Directory = file.read_be()?;

            // Read config
            let mut config = vec![0u8; directory.config_size as usize];
            file.seek(SeekFrom::Start(directory.config_offset))?;
            file.read_exact(&mut config)?;

            Ok(Self {
                id,
                primary_header,
                protected_header: Some(protected_header),
                config: Some(config),
                digest_table: None,
                directory: Some(directory),
                path: path.to_path_buf(),
                file_size: std::fs::metadata(&path)?.len(),
            })
        } else {
            Ok(Self {
                id,
                primary_header,
                protected_header: None,
                config: None,
                digest_table: None,
                directory: None,
                path: path.to_path_buf(),
                file_size: std::fs::metadata(&path)?.len(),
            })
        }
    }

    /// Modify the password and re-encrypt all encrypted sections. This doesn't
    /// re-encrypt the clusters because they are encrypted with the cluster key.
    pub fn change_password(&self, _old_password: String, new_password: String) -> Result<()> {
        // Create the cipher and a RNG for the nonces
        let _cipher = new_key(new_password);
        let _rng = rand::thread_rng();

        todo!()
    }

    /// Convert a qcow image into a goldboot image.
    pub fn convert<F: Fn(u64, u64) -> ()>(
        source: &Qcow3,
        name: String,
        config: Vec<u8>,
        password: Option<String>,
        dest: impl AsRef<Path>,
        progress: F,
    ) -> Result<ImageHandle> {
        info!("Exporting storage to goldboot image");

        let mut dest_file = File::create(&dest)?;
        let mut source_file = File::open(&source.path)?;

        // Prepare cipher and RNG if the image header should be encrypted
        let header_cipher = new_key(password.clone().unwrap_or("".to_string()));
        let mut rng = rand::thread_rng();

        // Prepare directory
        let mut directory = Directory {
            protected_nonce: rng.gen::<[u8; 12]>(),
            protected_size: 0,
            config_nonce: rng.gen::<[u8; 12]>(),
            config_offset: 0,
            config_size: 0,
            digest_table_nonce: rng.gen::<[u8; 12]>(),
            digest_table_offset: 0,
            digest_table_size: 0,
        };

        // Prepare primary header
        let mut primary_header = PrimaryHeader {
            version: 1,
            arch: ImageArch::Amd64, // TODO
            size: source.header.size,
            directory_nonce: rng.gen::<[u8; 12]>(),
            directory_offset: 0,
            directory_size: 0,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            encryption_type: if password.is_some() {
                HeaderEncryptionType::Aes256
            } else {
                HeaderEncryptionType::None
            },
            name: [0u8; 64],
        };

        primary_header.name[0..name.len()].copy_from_slice(&name.clone().as_bytes()[..]);

        // Prepare protected header
        let mut protected_header = ProtectedHeader {
            block_size: source.header.cluster_size() as u32,
            cluster_count: source.count_clusters()? as u32,
            cluster_compression: ClusterCompressionType::Zstd,
            cluster_encryption: if password.is_some() {
                ClusterEncryptionType::Aes256
            } else {
                ClusterEncryptionType::None
            },
            cluster_key: rng.gen::<[u8; 32]>(),
            nonce_count: 0,
            nonce_table: vec![],
        };

        if password.is_some() {
            protected_header.nonce_count = protected_header.cluster_count;
            protected_header.nonce_table = (0..protected_header.cluster_count)
                .map(|_| rng.gen::<[u8; 12]>())
                .collect();
        }

        // Load the cluster cipher we just generated
        let cluster_cipher =
            Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&protected_header.cluster_key));

        // Write primary header (we'll overwrite it at the end)
        dest_file.seek(SeekFrom::Start(0))?;
        primary_header.write(&mut dest_file)?;

        // Write protected header
        {
            debug!("Writing: {:?}", &protected_header);
            let mut protected_header_bytes = Cursor::new(Vec::new());
            protected_header.write(&mut protected_header_bytes)?;

            let protected_header_bytes = match primary_header.encryption_type {
                HeaderEncryptionType::None => protected_header_bytes.into_inner(),
                HeaderEncryptionType::Aes256 => header_cipher.encrypt(
                    Nonce::from_slice(&directory.protected_nonce),
                    protected_header_bytes.into_inner()[..].as_ref(),
                )?,
            };

            directory.protected_size = protected_header_bytes.len() as u32;
            dest_file.write_all(&protected_header_bytes)?;
        }

        // Write config
        {
            let config_bytes = match primary_header.encryption_type {
                HeaderEncryptionType::None => config.clone(),
                HeaderEncryptionType::Aes256 => header_cipher
                    .encrypt(Nonce::from_slice(&directory.config_nonce), config.as_ref())?,
            };

            directory.config_offset = dest_file.stream_position()?;
            directory.config_size = config_bytes.len() as u32;
            dest_file.write_all(&config_bytes)?;
        }

        // Prepare the digest table
        let mut digest_table = DigestTable {
            digest_count: protected_header.cluster_count,
            digest_table: vec![],
        };

        // Track the offset into the data
        let mut block_offset = 0;

        // Track cluster ordinal so we can lookup cluster nonces later
        let mut cluster_count = 0;

        // Track the cluster offset in the image file
        let mut cluster_offset = dest_file.stream_position()?;

        // Read from the qcow2 and write the clusters
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

                        // Compute hash of the block which will be used when writing the block later
                        let digest = Sha256::new().chain_update(&cluster.data).finalize();

                        digest_table.digest_table.push(DigestTableEntry {
                            digest: digest.into(),
                            block_offset,
                            cluster_offset,
                        });

                        // Perform compression
                        cluster.data = match protected_header.cluster_compression {
                            ClusterCompressionType::None => cluster.data,
                            ClusterCompressionType::Zstd => {
                                zstd::encode_all(std::io::Cursor::new(cluster.data), 0)?
                            }
                        };

                        // Perform encryption
                        cluster.data = match protected_header.cluster_encryption {
                            ClusterEncryptionType::None => cluster.data,
                            ClusterEncryptionType::Aes256 => cluster_cipher.encrypt(
                                Nonce::from_slice(&protected_header.nonce_table[cluster_count]),
                                cluster.data.as_ref(),
                            )?,
                        };

                        cluster.size = cluster.data.len() as u32;

                        // Write the cluster
                        trace!(
                            "Writing {} byte cluster to: {}",
                            cluster.size,
                            cluster_offset
                        );
                        cluster.write(&mut dest_file)?;

                        // Advance offset
                        cluster_offset += 4; // size
                        cluster_offset += cluster.size as u64;
                        cluster_count += 1;
                    }
                    block_offset += source.header.cluster_size();
                    progress(source.header.cluster_size(), source.header.size);
                }
            } else {
                block_offset +=
                    source.header.cluster_size() * source.header.l2_entries_per_cluster();
                progress(
                    source.header.cluster_size() * source.header.l2_entries_per_cluster(),
                    source.header.size,
                );
            }
        }

        // Write the completed digest table
        {
            let mut digest_table_bytes = Cursor::new(Vec::new());
            digest_table.write(&mut digest_table_bytes)?;

            let digest_table_bytes = match primary_header.encryption_type {
                HeaderEncryptionType::None => digest_table_bytes.into_inner(),
                HeaderEncryptionType::Aes256 => header_cipher.encrypt(
                    Nonce::from_slice(&directory.digest_table_nonce),
                    digest_table_bytes.into_inner()[..].as_ref(),
                )?,
            };

            directory.digest_table_offset = dest_file.stream_position()?;
            directory.digest_table_size = digest_table_bytes.len() as u32;
            dest_file.write_all(&digest_table_bytes)?;
        }

        // Write the completed directory
        {
            let mut directory_bytes = Cursor::new(Vec::new());
            directory.write(&mut directory_bytes)?;

            let directory_bytes = match primary_header.encryption_type {
                HeaderEncryptionType::None => directory_bytes.into_inner(),
                HeaderEncryptionType::Aes256 => header_cipher.encrypt(
                    Nonce::from_slice(&primary_header.directory_nonce),
                    directory_bytes.into_inner()[..].as_ref(),
                )?,
            };

            primary_header.directory_offset = dest_file.stream_position()?;
            primary_header.directory_size = directory_bytes.len() as u32;
            dest_file.write_all(&directory_bytes)?;
        }

        // Write the completed primary header
        dest_file.seek(SeekFrom::Start(0))?;
        primary_header.write(&mut dest_file)?;

        Ok(ImageHandle {
            id: compute_id(dest.as_ref())?,
            primary_header,
            protected_header: Some(protected_header),
            config: Some(config),
            digest_table: Some(digest_table),
            directory: Some(directory),
            path: dest.as_ref().to_path_buf(),
            file_size: std::fs::metadata(&dest)?.len(),
        })
    }

    /// TODO multi threaded WriteWorkers
    /// TODO write backup GPT header

    /// Write the image contents out to disk.
    pub fn write<F: Fn(u64, u64) -> ()>(&self, dest: impl AsRef<Path>, progress: F) -> Result<()> {
        if self.protected_header.is_none() || self.digest_table.is_none() {
            bail!("Image not loaded");
        }

        let protected_header = self.protected_header.clone().unwrap();
        let digest_table = self.digest_table.clone().unwrap().digest_table;

        let cluster_cipher =
            Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&protected_header.cluster_key));

        info!("Writing image");

        let mut dest = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .read(true)
            .open(dest)?;

        // Open the cluster table for reading
        let mut cluster_table = BufReader::new(File::open(&self.path)?);

        // Extend the file if necessary
        // TODO stream_len?
        if dest.metadata()?.len() < self.primary_header.size {
            dest.set_len(self.primary_header.size)?;
        }

        let mut block = vec![0u8; protected_header.block_size as usize];

        // Write all of the clusters that have changed
        for i in 0..protected_header.cluster_count as usize {
            // Load digest table entry
            let entry = &digest_table[i];

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
                cluster.data = match protected_header.cluster_encryption {
                    ClusterEncryptionType::None => cluster.data,
                    ClusterEncryptionType::Aes256 => cluster_cipher.decrypt(
                        Nonce::from_slice(&protected_header.nonce_table[i]),
                        cluster.data.as_ref(),
                    )?,
                };

                // Reverse compression
                cluster.data = match protected_header.cluster_compression {
                    ClusterCompressionType::None => cluster.data,
                    ClusterCompressionType::Zstd => {
                        zstd::decode_all(std::io::Cursor::new(&cluster.data))?
                    }
                };

                // Write the cluster to the block
                dest.seek(SeekFrom::Start(entry.block_offset))?;
                dest.write_all(&cluster.data)?;
            }

            progress(
                protected_header.block_size as u64,
                protected_header.cluster_count as u64 * protected_header.block_size as u64,
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha1::Sha1;

    #[test]
    fn convert_small_qcow2_to_unencrypted_image() -> Result<()> {
        let tmp = tempfile::tempdir()?;

        // Convert the test qcow2
        let image = ImageHandle::convert(
            &Qcow3::open("test/small.qcow2")?,
            String::from("Test"),
            vec![],
            None,
            tmp.path().join("small.gb"),
            |_, _| {},
        )?;

        // Try to open the image we just converted
        let mut loaded_image = ImageHandle::open(tmp.path().join("small.gb"))?;
        assert_eq!(loaded_image.primary_header, image.primary_header);
        assert_eq!(loaded_image.protected_header, image.protected_header);

        // Try to load all sections
        assert_eq!(loaded_image.digest_table, None);
        loaded_image.load(None)?;
        assert_eq!(loaded_image.digest_table.unwrap().digest_count, 2);

        // Check raw content
        image.write(tmp.path().join("small.raw"), |_, _| {})?;
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

    #[test]
    fn convert_small_qcow2_to_encrypted_image() -> Result<()> {
        let tmp = tempfile::tempdir()?;

        // Convert the test qcow2
        let image = ImageHandle::convert(
            &Qcow3::open("test/small.qcow2")?,
            String::from("Test"),
            vec![],
            Some(String::from("1234")),
            tmp.path().join("small.gb"),
            |_, _| {},
        )?;

        // Try to open the image
        let mut loaded_image = ImageHandle::open(tmp.path().join("small.gb"))?;
        assert_eq!(loaded_image.primary_header, image.primary_header);
        assert_eq!(loaded_image.protected_header, None);

        // Try to load all sections
        loaded_image.load(Some("1234".to_string()))?;

        // Check raw content
        image.write(tmp.path().join("small.raw"), |_, _| {})?;
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
