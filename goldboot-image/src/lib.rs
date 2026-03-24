//!

use crate::qcow::Qcow3;
use aes_gcm::{Aes256Gcm, Key, KeyInit, Nonce, aead::Aead};
use anyhow::{Context, Result, bail};
use binrw::{BinRead, BinReaderExt, BinWrite};
use rand::Rng;
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{BufReader, Cursor, Read, Seek, SeekFrom, Write},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
use strum::{Display, EnumIter};
use tracing::{debug, info, trace};

pub mod qcow;

/// Compute a CRC32 checksum using the IEEE polynomial (same as used by GPT).
fn crc32(data: &[u8]) -> u32 {
    // Build the lookup table
    let mut table = [0u32; 256];
    for i in 0..256u32 {
        let mut c = i;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xedb88320 ^ (c >> 1) } else { c >> 1 };
        }
        table[i as usize] = c;
    }
    let mut crc: u32 = 0xffffffff;
    for &b in data {
        crc = table[((crc ^ b as u32) & 0xff) as usize] ^ (crc >> 8);
    }
    crc ^ 0xffffffff
}

/// Read the primary GPT header from `dest`, construct the backup GPT header and
/// partition entries, then write them to the correct location at the end of the
/// disk.
///
/// The backup GPT header is a copy of the primary with:
/// - `MyLBA` set to the last LBA of the disk
/// - `AlternateLBA` set to LBA 1 (the primary)
/// - `PartitionEntryLBA` set to `MyLBA - 32` (32 sectors before the backup header)
/// - `HeaderCRC32` recomputed (with the field zeroed while computing)
///
/// The backup partition entry array is an identical copy of the primary entries
/// placed immediately before the backup header.
fn fixup_backup_gpt(dest: &mut (impl Read + Write + Seek)) -> Result<()> {
    // ---- read the primary GPT header (LBA 1 = offset 512) ----
    dest.seek(SeekFrom::Start(512))?;
    let mut hdr = [0u8; 512];
    dest.read_exact(&mut hdr)?;

    // Verify GPT signature "EFI PART"
    if &hdr[0..8] != b"EFI PART" {
        trace!("No GPT signature found, skipping backup GPT fixup");
        return Ok(());
    }

    // Parse fields (all little-endian)
    let header_size    = u32::from_le_bytes(hdr[12..16].try_into().unwrap()) as usize;
    let my_lba         = u64::from_le_bytes(hdr[24..32].try_into().unwrap());
    let alternate_lba  = u64::from_le_bytes(hdr[32..40].try_into().unwrap());
    let part_entry_lba = u64::from_le_bytes(hdr[72..80].try_into().unwrap());
    let num_entries    = u32::from_le_bytes(hdr[80..84].try_into().unwrap());
    let entry_size     = u32::from_le_bytes(hdr[84..88].try_into().unwrap()) as usize;

    // Sanity checks
    if my_lba != 1 || alternate_lba == 0 || header_size < 92 || entry_size == 0 || num_entries == 0 {
        trace!(my_lba, alternate_lba, header_size, "GPT header looks invalid, skipping backup fixup");
        return Ok(());
    }

    // ---- read the primary partition entries ----
    dest.seek(SeekFrom::Start(part_entry_lba * 512))?;
    let entries_size = num_entries as usize * entry_size;
    let mut entries = vec![0u8; entries_size];
    dest.read_exact(&mut entries)?;

    // ---- compute disk size from the alternate LBA recorded in primary header ----
    let disk_last_lba = alternate_lba; // where primary said backup lives
    let backup_entries_lba = disk_last_lba - 32;

    // ---- build the backup header ----
    let mut backup_hdr = hdr[..header_size].to_vec();
    backup_hdr.resize(header_size, 0);

    // Swap MyLBA and AlternateLBA
    backup_hdr[24..32].copy_from_slice(&disk_last_lba.to_le_bytes());
    backup_hdr[32..40].copy_from_slice(&my_lba.to_le_bytes());

    // Point PartitionEntryLBA to the backup entries location
    backup_hdr[72..80].copy_from_slice(&backup_entries_lba.to_le_bytes());

    // Recompute partition entries CRC32 (same entries, same data)
    let entries_crc = crc32(&entries);
    backup_hdr[88..92].copy_from_slice(&entries_crc.to_le_bytes());

    // Recompute header CRC32 (zero out the CRC field first)
    backup_hdr[16..20].copy_from_slice(&[0u8; 4]);
    let header_crc = crc32(&backup_hdr);
    backup_hdr[16..20].copy_from_slice(&header_crc.to_le_bytes());

    // ---- write backup partition entries ----
    dest.seek(SeekFrom::Start(backup_entries_lba * 512))?;
    dest.write_all(&entries)?;

    // ---- write backup header ----
    let mut backup_sector = [0u8; 512];
    backup_sector[..backup_hdr.len()].copy_from_slice(&backup_hdr);
    dest.seek(SeekFrom::Start(disk_last_lba * 512))?;
    dest.write_all(&backup_sector)?;

    debug!(disk_last_lba, backup_entries_lba, "Wrote backup GPT header");
    Ok(())
}

trait ReadSeek: Read + Seek {}
impl ReadSeek for BufReader<File> {}
impl ReadSeek for Cursor<Vec<u8>> {}

/// Returns the number of bytes of available memory, or 0 if unknown.
fn available_memory() -> u64 {
    #[cfg(target_os = "linux")]
    {
        if let Ok(s) = std::fs::read_to_string("/proc/meminfo") {
            if let Some(kb) = s
                .lines()
                .find(|l| l.starts_with("MemAvailable:"))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|v| v.parse::<u64>().ok())
            {
                return kb * 1024;
            }
        }
        0
    }
    #[cfg(target_os = "windows")]
    {
        use std::mem;
        #[allow(non_camel_case_types)]
        #[repr(C)]
        struct MEMORYSTATUSEX {
            dw_length: u32,
            dw_memory_load: u32,
            ull_total_phys: u64,
            ull_avail_phys: u64,
            ull_total_page_file: u64,
            ull_avail_page_file: u64,
            ull_total_virtual: u64,
            ull_avail_virtual: u64,
            ull_avail_extended_virtual: u64,
        }
        extern "system" {
            fn GlobalMemoryStatusEx(lp_buffer: *mut MEMORYSTATUSEX) -> i32;
        }
        let mut stat: MEMORYSTATUSEX = unsafe { mem::zeroed() };
        stat.dw_length = mem::size_of::<MEMORYSTATUSEX>() as u32;
        if unsafe { GlobalMemoryStatusEx(&mut stat) } != 0 {
            return stat.ull_avail_phys;
        }
        0
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        0
    }
}

/// Supported system architectures for goldboot images.
#[derive(
    BinRead, BinWrite, Clone, Copy, Serialize, Deserialize, Debug, PartialEq, Eq, EnumIter, Display,
)]
#[brw(repr(u8))]
pub enum ImageArch {
    Amd64,
    Arm64,
    I386,
    Mips,
    Mips64,
    S390x,
}

impl ImageArch {
    pub fn as_github_string(&self) -> String {
        match self {
            ImageArch::Amd64 => "x86_64",
            ImageArch::Arm64 => todo!(),
            ImageArch::I386 => todo!(),
            ImageArch::Mips => todo!(),
            ImageArch::Mips64 => todo!(),
            ImageArch::S390x => todo!(),
        }
        .to_string()
    }
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

impl std::fmt::Debug for ImageHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageHandle")
            .field("primary_header", &self.primary_header)
            .field("path", &self.path)
            .field("file_size", &self.file_size)
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

/// The cluster compression algorithm.
#[derive(BinRead, BinWrite, Debug, Eq, PartialEq, Clone)]
#[brw(repr(u8))]
pub enum ClusterCompressionType {
    /// Clusters will not be compressed
    None = 0,

    /// Clusters will be compressed with Z standard
    Zstd = 1,
}

/// The cluster encryption algorithm.
#[derive(BinRead, BinWrite, Debug, Eq, PartialEq, Clone)]
#[brw(repr(u8))]
pub enum ClusterEncryptionType {
    /// Clusters are not encrypted
    None = 0,

    /// Clusters are encrypted with AES256 GCM after compression
    Aes256 = 1,
}

/// Algorithm used to encrypt the header.
#[derive(BinRead, BinWrite, Debug, Eq, PartialEq, Clone)]
#[brw(repr(u8))]
pub enum HeaderEncryptionType {
    /// Header is not encrypted
    None = 0,

    /// Header is encrypted with AES256 GCM
    Aes256 = 1,
}

/// Metadata about an element within this image.
#[derive(BinRead, BinWrite, Debug, Eq, PartialEq)]
#[brw(big)]
pub struct ElementHeader {
    /// Length of the os field in bytes
    pub os_length: u8,

    /// Element OS type
    #[br(count = os_length)]
    pub os: Vec<u8>,

    /// Length of the name field in bytes
    pub name_length: u8,

    /// Element name
    #[br(count = name_length)]
    pub name: Vec<u8>,
}

impl ElementHeader {
    pub fn name(&self) -> String {
        String::from_utf8_lossy(&self.name).into_owned()
    }

    pub fn os(&self) -> String {
        String::from_utf8_lossy(&self.os).into_owned()
    }

    pub fn new(os: &str, name: &str) -> Result<ElementHeader> {
        Ok(ElementHeader {
            os_length: u8::try_from(os.len()).context("OS string too long")?,
            os: os.as_bytes().to_vec(),
            name_length: u8::try_from(name.len()).context("Name string too long")?,
            name: name.as_bytes().to_vec(),
        })
    }
}

/// Contains metadata which is always plaintext. Anything potentially useful to
/// an attacker should instead reside in the protected header unless the user
/// may want to read it without decrypting the image first.
#[derive(BinRead, BinWrite, Debug, Eq, PartialEq)]
#[brw(magic = b"\xc0\x1d\xb0\x01", big)]
pub struct PrimaryHeader {
    /// Format version
    #[br(assert(version == 1))]
    pub version: u8,

    /// Total size of all blocks combined in bytes
    pub size: u64,

    /// Image creation time
    pub timestamp: u64,

    /// The encryption type for metadata
    pub encryption_type: HeaderEncryptionType,

    /// Number of elements in this image
    pub element_count: u8,

    #[br(count = element_count)]
    pub elements: Vec<ElementHeader>,

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
        let parts: Vec<String> = self.elements.iter().map(|element| element.name()).collect();
        parts.join(" / ")
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
    /// Size of the cluster in bytes
    pub size: u32,

    /// Cluster data (might be compressed and encrypted)
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
    use sha2::Digest;
    let mut file = File::open(&path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
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

                let directory_bytes = cipher
                    .decrypt(
                        Nonce::from_slice(&self.primary_header.directory_nonce),
                        directory_bytes.as_ref(),
                    )
                    .map_err(|e| anyhow::anyhow!("decryption failed: {e}"))?;
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

                let protected_header_bytes = cipher
                    .decrypt(
                        Nonce::from_slice(&directory.protected_nonce),
                        protected_header_bytes.as_ref(),
                    )
                    .map_err(|e| anyhow::anyhow!("decryption failed: {e}"))?;
                Cursor::new(protected_header_bytes).read_be()?
            }
        };

        // Load the digest table
        file.seek(SeekFrom::Start(directory.digest_table_offset))?;
        let digest_table: DigestTable = match self.primary_header.encryption_type {
            HeaderEncryptionType::None => file.read_be()?,
            HeaderEncryptionType::Aes256 => {
                let mut digest_table_bytes = vec![0u8; directory.digest_table_size as usize];
                file.read_exact(&mut digest_table_bytes)?;

                let digest_table_bytes = cipher
                    .decrypt(
                        Nonce::from_slice(&directory.digest_table_nonce),
                        digest_table_bytes.as_ref(),
                    )
                    .map_err(|e| anyhow::anyhow!("decryption failed: {e}"))?;
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

        debug!(path = ?path, "Opening image");

        // Read primary header (always plaintext)
        let primary_header: PrimaryHeader = file.read_be()?;
        debug!(primary_header = ?primary_header, "Primary header");

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

            Ok(Self {
                id,
                primary_header,
                protected_header: Some(protected_header),
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

        todo!()
    }

    /// TODO multi threaded WriteWorkers

    /// Write the image contents out to disk.
    ///
    /// The progress callback receives `(cluster_index, state)` for each cluster:
    /// - `None`        — cluster is dirty and is now being written
    /// - `Some(true)`  — cluster was dirty and has been written
    /// - `Some(false)` — cluster was already up to date, no write needed
    pub fn write<F: Fn(usize, Option<bool>) -> ()>(&self, dest: impl AsRef<Path>, progress: F) -> Result<()> {
        if self.protected_header.is_none() || self.digest_table.is_none() {
            bail!("Image not loaded");
        }

        let protected_header = self.protected_header.clone().unwrap();
        let digest_table = self.digest_table.clone().unwrap().digest_table;

        let cluster_cipher =
            Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&protected_header.cluster_key));

        let dest = dest.as_ref();
        info!(image = ?self, dest = ?dest, "Writing goldboot image");

        let mut dest = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .read(true)
            .open(dest)?;

        // Open the cluster table for reading. If there's enough available memory
        // (with a 2% buffer), load the entire image into memory for faster access.
        let available_mem = available_memory();
        let mut cluster_table: Box<dyn ReadSeek> =
            if available_mem > 0 && self.file_size <= available_mem * 98 / 100 {
                debug!(file_size = self.file_size, available_mem, "Loading entire image into memory");
                Box::new(Cursor::new(std::fs::read(&self.path)?))
            } else {
                Box::new(BufReader::new(File::open(&self.path)?))
            };

        // Extend regular files if necessary
        // TODO also check size of block devices
        let dest_metadata = dest.metadata()?;
        if dest_metadata.is_file() && dest_metadata.len() < self.primary_header.size {
            dest.set_len(self.primary_header.size)?;
        }

        let mut block = vec![0u8; protected_header.block_size as usize];

        // Write all of the clusters that have changed
        for i in 0..protected_header.cluster_count as usize {
            // Load digest table entry
            let entry = &digest_table[i];

            // Jump to the block corresponding to the cluster
            dest.seek(SeekFrom::Start(entry.block_offset))?;

            // Hash the block to avoid unnecessary writes
            let hash: [u8; 32] = match dest.read_exact(&mut block) {
                Ok(_) => Sha256::new().chain_update(&block).finalize().into(),
                Err(_) => {
                    // TODO check for EOF error
                    [0u8; 32]
                }
            };

            let is_dirty = hash != entry.digest;

            if is_dirty {
                // Signal that this cluster is now being written
                progress(i, None);

                // Read cluster
                cluster_table.seek(SeekFrom::Start(entry.cluster_offset))?;
                let mut cluster: Cluster = cluster_table.read_be()?;

                trace!(
                    cluster_size = cluster.size,
                    cluster_offset = entry.cluster_offset,
                    "Read dirty cluster",
                );

                // Reverse encryption
                cluster.data = match protected_header.cluster_encryption {
                    ClusterEncryptionType::None => cluster.data,
                    ClusterEncryptionType::Aes256 => cluster_cipher
                        .decrypt(
                            Nonce::from_slice(&protected_header.nonce_table[i]),
                            cluster.data.as_ref(),
                        )
                        .map_err(|e| anyhow::anyhow!("decryption failed: {e}"))?,
                };

                // Reverse compression
                cluster.data = match protected_header.cluster_compression {
                    ClusterCompressionType::None => cluster.data,
                    ClusterCompressionType::Zstd => {
                        zstd::decode_all(std::io::Cursor::new(&cluster.data))?
                    }
                };

                trace!(
                    block_offset = entry.block_offset,
                    block_size = cluster.data.len(),
                    "Writing block",
                );

                // Write the cluster to the block
                dest.seek(SeekFrom::Start(entry.block_offset))?;
                dest.write_all(&cluster.data)?;
            }

            progress(i, Some(is_dirty));
        }

        fixup_backup_gpt(&mut dest)?;

        Ok(())
    }

    /// Verify the image contents on disk by reading and hashing each block.
    ///
    /// The progress callback receives `(cluster_index, verified)`:
    /// - `None`       — cluster is now being read/hashed
    /// - `Some(true)` — cluster hash matched
    /// - `Some(false)`— cluster hash did not match (corruption detected)
    pub fn verify<F: Fn(usize, Option<bool>) -> ()>(&self, dest: impl AsRef<Path>, progress: F) -> Result<()> {
        if self.protected_header.is_none() || self.digest_table.is_none() {
            bail!("Image not loaded");
        }

        let protected_header = self.protected_header.clone().unwrap();
        let digest_table = self.digest_table.clone().unwrap().digest_table;

        let mut dest = std::fs::OpenOptions::new()
            .read(true)
            .open(dest)?;

        let mut block = vec![0u8; protected_header.block_size as usize];

        for i in 0..protected_header.cluster_count as usize {
            let entry = &digest_table[i];

            progress(i, None);

            dest.seek(SeekFrom::Start(entry.block_offset))?;
            let hash: [u8; 32] = match dest.read_exact(&mut block) {
                Ok(_) => Sha256::new().chain_update(&block).finalize().into(),
                Err(_) => [0u8; 32],
            };

            progress(i, Some(hash == entry.digest));
        }

        Ok(())
    }

    /// Convert a qcow image into a goldboot image.
    pub fn from_qcow<F: Fn(u64, u64) -> ()>(
        metadata: Vec<ElementHeader>,
        source: &Qcow3,
        dest: impl AsRef<Path>,
        password: Option<String>,
        progress: F,
    ) -> Result<ImageHandle> {
        info!(qcow = ?source, "Converting qcow image to goldboot image");

        let mut dest_file = File::create(&dest)?;
        let mut source_file = File::open(&source.path)?;

        // Prepare cipher and RNG if the image header should be encrypted
        let header_cipher = new_key(password.clone().unwrap_or("".to_string()));
        let mut rng = rand::rng();

        // Prepare directory
        let mut directory = Directory {
            protected_nonce: rng.random::<[u8; 12]>(),
            protected_size: 0,
            digest_table_nonce: rng.random::<[u8; 12]>(),
            digest_table_offset: 0,
            digest_table_size: 0,
        };

        // Prepare primary header
        let mut primary_header = PrimaryHeader {
            version: 1,
            arch: ImageArch::Amd64,   // TODO
            size: source.header.size, // TODO this is aligned to the cluster size?
            directory_nonce: rng.random::<[u8; 12]>(),
            directory_offset: 0,
            directory_size: 0,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            encryption_type: if password.is_some() {
                HeaderEncryptionType::Aes256
            } else {
                HeaderEncryptionType::None
            },
            element_count: u8::try_from(metadata.len()).context("Too many elements")?,
            elements: metadata,
        };

        // Prepare protected header
        let mut protected_header = ProtectedHeader {
            block_size: source.header.cluster_size() as u32,
            cluster_count: source.count_clusters()? as u32,
            cluster_compression: ClusterCompressionType::None, // TODO
            cluster_encryption: if password.is_some() {
                ClusterEncryptionType::Aes256
            } else {
                ClusterEncryptionType::None
            },
            cluster_key: rng.random::<[u8; 32]>(),
            nonce_count: 0,
            nonce_table: vec![],
        };

        if password.is_some() {
            protected_header.nonce_count = protected_header.cluster_count;
            protected_header.nonce_table = (0..protected_header.cluster_count)
                .map(|_| rng.random::<[u8; 12]>())
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
            debug!(protected_header = ?protected_header, "Writing protected header");
            let mut protected_header_bytes = Cursor::new(Vec::new());
            protected_header.write(&mut protected_header_bytes)?;

            let protected_header_bytes = match primary_header.encryption_type {
                HeaderEncryptionType::None => protected_header_bytes.into_inner(),
                HeaderEncryptionType::Aes256 => header_cipher
                    .encrypt(
                        Nonce::from_slice(&directory.protected_nonce),
                        protected_header_bytes.into_inner()[..].as_ref(),
                    )
                    .map_err(|e| anyhow::anyhow!("encryption failed: {e}"))?,
            };

            directory.protected_size = protected_header_bytes.len() as u32;
            dest_file.write_all(&protected_header_bytes)?;
        }

        // Prepare the digest table
        let mut digest_table = DigestTable {
            digest_count: protected_header.cluster_count,
            digest_table: vec![],
        };

        // Track the offset into the data
        let mut block_offset: u64 = 0;

        // Track cluster ordinal so we can lookup cluster nonces later
        let mut cluster_count = 0;

        // Track the cluster offset in the image file
        let mut cluster_offset = dest_file.stream_position()?;

        // Read from the qcow2 and write the clusters
        for l1_entry in &source.l1_table {
            if let Some(l2_table) = l1_entry.read_l2(&mut source_file, source.header.cluster_bits) {
                for l2_entry in l2_table {
                    if l2_entry.is_allocated() {
                        // Start building the cluster
                        let mut cluster = Cluster {
                            // The resulting size gets updated after we compress/encrypt
                            size: 0,
                            data: l2_entry.read_contents(
                                &mut source_file,
                                source.header.cluster_size(),
                                source.header.compression_type,
                            )?,
                        };

                        // TODO image size may exceed usize
                        cluster
                            .data
                            .truncate((primary_header.size - block_offset) as usize);

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
                            ClusterEncryptionType::Aes256 => cluster_cipher
                                .encrypt(
                                    Nonce::from_slice(&protected_header.nonce_table[cluster_count]),
                                    cluster.data.as_ref(),
                                )
                                .map_err(|e| anyhow::anyhow!("encryption failed: {e}"))?,
                        };

                        cluster.size = cluster.data.len() as u32;

                        // Write the cluster
                        trace!(
                            cluster_size = cluster.size,
                            cluster_offset = cluster_offset,
                            "Recording cluster",
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
                HeaderEncryptionType::Aes256 => header_cipher
                    .encrypt(
                        Nonce::from_slice(&directory.digest_table_nonce),
                        digest_table_bytes.into_inner()[..].as_ref(),
                    )
                    .map_err(|e| anyhow::anyhow!("encryption failed: {e}"))?,
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
                HeaderEncryptionType::Aes256 => header_cipher
                    .encrypt(
                        Nonce::from_slice(&primary_header.directory_nonce),
                        directory_bytes.into_inner()[..].as_ref(),
                    )
                    .map_err(|e| anyhow::anyhow!("encryption failed: {e}"))?,
            };

            primary_header.directory_offset = dest_file.stream_position()?;
            primary_header.directory_size = directory_bytes.len() as u32;
            dest_file.write_all(&directory_bytes)?;
        }

        // Write the completed primary header
        dest_file.seek(SeekFrom::Start(0))?;
        primary_header.write(&mut dest_file)?;

        Ok(ImageHandle {
            id: compute_id(&dest)?,
            primary_header,
            protected_header: Some(protected_header),
            digest_table: Some(digest_table),
            directory: Some(directory),
            file_size: std::fs::metadata(&dest)?.len(),
            path: dest.as_ref().to_path_buf(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::*;
    use sha1::Sha1;
    use test_log::test;

    #[test]
    fn convert_random_data() -> Result<()> {
        let tmp = tempfile::tempdir()?;
        let mut rng = rand::rng();

        // Generate file with random contents; size must be cluster-aligned because
        // qemu-img rounds the virtual disk size up to the next cluster boundary
        // (65536 bytes), so the output will always be that size.
        let size: usize = 65536;
        let raw: Vec<u8> = (0..size).map(|_| rng.random()).collect();

        // Write out for qemu-img
        std::fs::write(tmp.path().join("file.raw"), &raw)?;

        // Convert with qemu-img
        let status = Command::new("qemu-img")
            .arg("convert")
            .arg("-f")
            .arg("raw")
            .arg("-O")
            .arg("qcow2")
            .arg(tmp.path().join("file.raw"))
            .arg(tmp.path().join("file.qcow2"))
            .status()?;
        assert!(status.success(), "qemu-img convert failed");

        // Convert the qcow2 to gb
        let image = ImageHandle::from_qcow(
            Vec::new(),
            &Qcow3::open(tmp.path().join("file.qcow2"))?,
            tmp.path().join("file.gb"),
            None,
            |_, _| {},
        )?;

        // Check raw content round-trips correctly
        image.write(tmp.path().join("output.raw"), |_, _| {})?;
        let output = std::fs::read(tmp.path().join("output.raw"))?;
        assert_eq!(raw, output);

        Ok(())
    }

    #[test]
    fn convert_small_qcow2_to_unencrypted_image() -> Result<()> {
        let tmp = tempfile::tempdir()?;

        // Convert the test qcow2
        let image = ImageHandle::from_qcow(
            Vec::new(),
            &Qcow3::open("test/small.qcow2")?,
            tmp.path().join("small.gb"),
            None,
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
        let image = ImageHandle::from_qcow(
            Vec::new(),
            &Qcow3::open("test/small.qcow2")?,
            tmp.path().join("small.gb"),
            Some("1234".to_string()),
            |_, _| {},
        )?;

        // Try to open the image
        let mut loaded_image = ImageHandle::open(tmp.path().join("small.gb"))?;
        assert_eq!(loaded_image.primary_header, image.primary_header);
        assert_eq!(loaded_image.protected_header, None);

        // Try to load all sections
        loaded_image.load(Some("1234".to_string()))?;
        assert!(loaded_image.protected_header.is_some());
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
    fn convert_zlib_compressed_qcow2() -> Result<()> {
        let tmp = tempfile::tempdir()?;

        let image = ImageHandle::from_qcow(
            Vec::new(),
            &Qcow3::open("test/compressed_zlib.qcow2")?,
            tmp.path().join("compressed_zlib.gb"),
            None,
            |_, _| {},
        )?;

        // Verify the image can be re-opened and loaded
        let mut loaded_image = ImageHandle::open(tmp.path().join("compressed_zlib.gb"))?;
        assert_eq!(loaded_image.primary_header, image.primary_header);
        loaded_image.load(None)?;

        // Check raw content round-trips correctly
        loaded_image.write(tmp.path().join("compressed_zlib.raw"), |_, _| {})?;
        assert_eq!(
            hex::encode(
                Sha1::new()
                    .chain_update(&std::fs::read(tmp.path().join("compressed_zlib.raw"))?)
                    .finalize()
            ),
            "ffa601d0d0e8a39af78cf9a80cb3072b1db87f9f"
        );

        Ok(())
    }

    #[test]
    fn convert_zstd_compressed_qcow2() -> Result<()> {
        let tmp = tempfile::tempdir()?;

        let image = ImageHandle::from_qcow(
            Vec::new(),
            &Qcow3::open("test/compressed_zstd.qcow2")?,
            tmp.path().join("compressed_zstd.gb"),
            None,
            |_, _| {},
        )?;

        // Verify the image can be re-opened and loaded
        let mut loaded_image = ImageHandle::open(tmp.path().join("compressed_zstd.gb"))?;
        assert_eq!(loaded_image.primary_header, image.primary_header);
        loaded_image.load(None)?;

        // Check raw content round-trips correctly
        loaded_image.write(tmp.path().join("compressed_zstd.raw"), |_, _| {})?;
        assert_eq!(
            hex::encode(
                Sha1::new()
                    .chain_update(&std::fs::read(tmp.path().join("compressed_zstd.raw"))?)
                    .finalize()
            ),
            "39598cbdb5264aa441bc7954f52055fa9666d5ab"
        );

        Ok(())
    }

    #[test]
    fn element_header_round_trip() -> Result<()> {
        let elem = ElementHeader::new("arch_linux", "root")?;
        assert_eq!(elem.os(), "arch_linux");
        assert_eq!(elem.name(), "root");
        assert_eq!(elem.os_length, 10);
        assert_eq!(elem.name_length, 4);
        Ok(())
    }

    #[test]
    fn image_with_elements_round_trip() -> Result<()> {
        let tmp = tempfile::tempdir()?;

        let elements = vec![
            ElementHeader::new("arch_linux", "root")?,
            ElementHeader::new("ubuntu", "data")?,
        ];

        let image = ImageHandle::from_qcow(
            elements,
            &Qcow3::open("test/small.qcow2")?,
            tmp.path().join("with_elements.gb"),
            None,
            |_, _| {},
        )?;

        assert_eq!(image.primary_header.element_count, 2);
        assert_eq!(image.primary_header.elements[0].os(), "arch_linux");
        assert_eq!(image.primary_header.elements[0].name(), "root");
        assert_eq!(image.primary_header.elements[1].os(), "ubuntu");
        assert_eq!(image.primary_header.elements[1].name(), "data");
        assert_eq!(image.primary_header.name(), "root / data");

        // Verify elements survive an open/load round-trip
        let loaded = ImageHandle::open(tmp.path().join("with_elements.gb"))?;
        assert_eq!(loaded.primary_header.element_count, 2);
        assert_eq!(loaded.primary_header.elements[0].os(), "arch_linux");
        assert_eq!(loaded.primary_header.elements[0].name(), "root");
        assert_eq!(loaded.primary_header.elements[1].os(), "ubuntu");
        assert_eq!(loaded.primary_header.elements[1].name(), "data");

        Ok(())
    }

    #[test]
    fn image_arch_try_from_string() -> Result<()> {
        assert_eq!(ImageArch::try_from("amd64".to_string())?, ImageArch::Amd64);
        assert_eq!(ImageArch::try_from("x86_64".to_string())?, ImageArch::Amd64);
        assert_eq!(ImageArch::try_from("arm64".to_string())?, ImageArch::Arm64);
        assert_eq!(ImageArch::try_from("aarch64".to_string())?, ImageArch::Arm64);
        assert_eq!(ImageArch::try_from("i386".to_string())?, ImageArch::I386);
        // Case insensitivity
        assert_eq!(ImageArch::try_from("AMD64".to_string())?, ImageArch::Amd64);
        // Unknown arch returns an error
        assert!(ImageArch::try_from("riscv64".to_string()).is_err());
        Ok(())
    }

    #[test]
    fn compute_id_is_stable() -> Result<()> {
        // The same file should produce the same ID on repeated calls
        let id1 = compute_id("test/small.qcow2")?;
        let id2 = compute_id("test/small.qcow2")?;
        assert_eq!(id1, id2);
        // IDs of different files must differ
        let id3 = compute_id("test/empty.qcow2")?;
        assert_ne!(id1, id3);
        // ID is a valid 64-char hex string (SHA256)
        assert_eq!(id1.len(), 64);
        assert!(id1.chars().all(|c| c.is_ascii_hexdigit()));
        Ok(())
    }

    #[test]
    fn write_fails_when_not_loaded() -> Result<()> {
        let tmp = tempfile::tempdir()?;

        // Create a real .gb file, then open it without calling load()
        let _image = ImageHandle::from_qcow(
            Vec::new(),
            &Qcow3::open("test/small.qcow2")?,
            tmp.path().join("small.gb"),
            None,
            |_, _| {},
        )?;
        let partial = ImageHandle::open(tmp.path().join("small.gb"))?;

        // digest_table is None after open() — write() must fail
        assert!(partial.digest_table.is_none());
        let result = partial.write(tmp.path().join("should_not_exist.raw"), |_, _| {});
        assert!(result.is_err());

        Ok(())
    }
}
