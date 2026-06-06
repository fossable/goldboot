use crate::qcow::Qcow3;
use aes_gcm::{Aes256Gcm, Key, KeyInit, Nonce, aead::Aead};
use anyhow::{Context, Result, bail};
use binrw::{BinRead, BinReaderExt, BinWrite};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs::File,
    io::{BufReader, Cursor, Read, Seek, SeekFrom, Write},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
use strum::{Display, EnumIter};
use tracing::{debug, info, trace};

pub mod qcow;

trait ReadSeek: Read + Seek {}
impl ReadSeek for BufReader<File> {}
impl ReadSeek for Cursor<Vec<u8>> {}

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

/// Maximum length of a name or tag (in bytes). Matches the registry's
/// on-disk component limit so the wire / file / header agree.
pub const MAX_REF_SEGMENT_LEN: usize = 64;

/// Validate a `<name>` or `<tag>` segment of an image reference.
/// Allowed: ASCII letters, digits, `.`, `-`, `_`. Rejects empty, `.`, `..`,
/// and anything containing path separators or NULs. 1–64 bytes.
pub fn validate_ref_segment(s: &str) -> Result<()> {
    if s.is_empty() {
        bail!("empty reference segment");
    }
    if s.len() > MAX_REF_SEGMENT_LEN {
        bail!("segment '{}' exceeds {} bytes", s, MAX_REF_SEGMENT_LEN);
    }
    if s == "." || s == ".." {
        bail!("reserved segment: '{}'", s);
    }
    for (i, b) in s.bytes().enumerate() {
        let ok = b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_');
        if !ok {
            bail!("invalid byte 0x{:02x} at position {} in '{}'", b, i, s);
        }
    }
    Ok(())
}

/// Validate a host segment. Looser than `validate_ref_segment` — also
/// allows `:` (so `localhost:8080` round-trips) but still rejects path
/// separators, NULs, and reserved names.
pub fn validate_host_segment(s: &str) -> Result<()> {
    if s.is_empty() {
        bail!("empty host");
    }
    if s.len() > 253 {
        bail!("host '{}' too long", s);
    }
    if s == "." || s == ".." {
        bail!("reserved host: '{}'", s);
    }
    for (i, b) in s.bytes().enumerate() {
        let ok = b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b':');
        if !ok {
            bail!("invalid byte 0x{:02x} at position {} in host '{}'", b, i, s);
        }
    }
    Ok(())
}

/// Strip any `http://` or `https://` prefix from a host string.
pub fn host_without_scheme(host: &str) -> &str {
    host.strip_prefix("https://")
        .or_else(|| host.strip_prefix("http://"))
        .unwrap_or(host)
}

/// A parsed image reference: `[scheme://][<host>/]<name>[:<tag>]`.
///
/// `name` is required; `host` and `tag` are both optional. The struct is
/// shape-only — it does not enforce a host for any particular operation.
/// Callers that *require* a host (push, pull) check `host.is_none()`
/// themselves and produce an appropriate error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageRef {
    /// Host, including any `http://` / `https://` scheme prefix if the
    /// user typed one. Use [`Self::host_bare`] for the scheme-free form
    /// and [`Self::host_or_local`] for library lookups.
    pub host: Option<String>,
    pub name: String,
    pub tag: Option<String>,
}

impl ImageRef {
    /// Build a bare-name reference (no host, no tag).
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            host: None,
            name: name.into(),
            tag: None,
        }
    }

    /// Builder: attach a host.
    pub fn with_host(mut self, host: impl Into<String>) -> Self {
        self.host = Some(host.into());
        self
    }

    /// Builder: attach a tag.
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = Some(tag.into());
        self
    }

    /// Parse a reference of the form `[scheme://][<host>/]<name>[:<tag>]`.
    ///
    /// Disambiguation rules:
    /// - The last `:` introduces a tag, **unless** what follows contains
    ///   `/` (in which case the `:` is part of a `host:port` and there's
    ///   no tag).
    /// - If a `/` remains after stripping the tag, everything before the
    ///   first `/` is the host.
    pub fn parse(reference: &str) -> Result<Self> {
        let (scheme_prefix, rest) = if let Some(rest) = reference.strip_prefix("http://") {
            ("http://", rest)
        } else if let Some(rest) = reference.strip_prefix("https://") {
            ("https://", rest)
        } else {
            ("", reference)
        };

        // Split off the tag (if any).
        let (ref_no_tag, tag): (&str, Option<String>) = if let Some(pos) = rest.rfind(':') {
            let after_colon = &rest[pos + 1..];
            if after_colon.contains('/') {
                (rest, None)
            } else {
                (&rest[..pos], Some(after_colon.to_string()))
            }
        } else {
            (rest, None)
        };

        // Split off the host (if any).
        let (host, name) = if let Some(slash) = ref_no_tag.find('/') {
            let host_part = &ref_no_tag[..slash];
            let name_part = &ref_no_tag[slash + 1..];
            (Some(format!("{scheme_prefix}{host_part}")), name_part)
        } else {
            if !scheme_prefix.is_empty() {
                bail!(
                    "reference '{}' has a scheme but no host before the name",
                    reference
                );
            }
            (None, ref_no_tag)
        };

        if name.is_empty() {
            bail!("reference '{}' is missing the image name", reference);
        }

        Ok(Self {
            host,
            name: name.to_string(),
            tag,
        })
    }

    /// Host with any `http://` / `https://` prefix removed.
    pub fn host_bare(&self) -> Option<&str> {
        self.host.as_deref().map(host_without_scheme)
    }

    /// Validate that `name` (and `tag`, when present) are well-formed.
    /// `host`, when present, is checked with [`validate_host_segment`].
    pub fn validate(&self) -> Result<()> {
        validate_ref_segment(&self.name).context("invalid image name")?;
        if let Some(tag) = &self.tag {
            validate_ref_segment(tag).context("invalid image tag")?;
        }
        if let Some(host) = self.host_bare() {
            validate_host_segment(host).context("invalid image host")?;
        }
        Ok(())
    }
}

impl std::fmt::Display for ImageRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (&self.host, &self.tag) {
            (Some(h), Some(t)) => write!(f, "{h}/{}:{t}", self.name),
            (Some(h), None) => write!(f, "{h}/{}", self.name),
            (None, Some(t)) => write!(f, "{}:{t}", self.name),
            (None, None) => f.write_str(&self.name),
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
#[derive(BinRead, BinWrite, Debug, Eq, PartialEq, Clone, Copy)]
#[brw(repr(u8))]
pub enum ClusterCompressionType {
    /// Clusters will not be compressed
    None = 0,

    /// Clusters will be compressed with Z standard
    Zstd = 1,
}

/// The cluster encryption algorithm.
#[derive(BinRead, BinWrite, Debug, Eq, PartialEq, Clone, Copy)]
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
    #[br(assert(version == 2))]
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

    /// Length of the name field in bytes
    pub name_length: u8,

    /// Image name. Part of the `<host>/<name>:<tag>` reference; intrinsic
    /// to the image's identity.
    #[br(count = name_length)]
    pub name: Vec<u8>,

    /// Length of the tag field in bytes
    pub tag_length: u8,

    /// Image tag. Part of the `<host>/<name>:<tag>` reference; intrinsic
    /// to the image's identity.
    #[br(count = tag_length)]
    pub tag: Vec<u8>,

    /// SHA256 over the cluster region only (the image's data blocks).
    /// Two images with identical payload share this value, even if their
    /// name/tag/timestamp differ. Informational — never used as a key.
    pub content_id: [u8; 32],

    /// Directory nonce
    pub directory_nonce: [u8; 12],

    /// The byte offset of the directory
    pub directory_offset: u64,

    /// The size of the directory in bytes
    pub directory_size: u32,
}

impl PrimaryHeader {
    /// Image name from the header.
    pub fn name_str(&self) -> String {
        String::from_utf8_lossy(&self.name).into_owned()
    }

    /// Image tag from the header.
    pub fn tag_str(&self) -> String {
        String::from_utf8_lossy(&self.tag).into_owned()
    }

    /// Hex-encoded content ID (SHA256 of the cluster region).
    pub fn content_id_hex(&self) -> String {
        hex::encode(self.content_id)
    }

    /// Display label joining the per-element names. Not an identifier —
    /// use `name_str()` for that.
    pub fn elements_label(&self) -> String {
        let parts: Vec<String> = self.elements.iter().map(|element| element.name()).collect();
        parts.join(" / ")
    }

    /// Parse a `PrimaryHeader` from the start of an in-memory byte slice.
    /// Useful for callers (e.g. the registry server) that already have the
    /// raw upload buffer and want to inspect the header without taking a
    /// dependency on `binrw` themselves.
    pub fn read_from_bytes(bytes: &[u8]) -> Result<Self> {
        let mut cursor = Cursor::new(bytes);
        Ok(cursor.read_be()?)
    }
}

/// Contains metadata which may be encrypted.
#[derive(BinRead, BinWrite, Eq, PartialEq, Clone)]
#[brw(big)]
pub struct ProtectedHeader {
    /// The size in bytes of each disk block
    pub block_size: u32,

    /// The number of populated clusters in this image
    pub cluster_count: u32,

    /// Compression type used on clusters
    pub cluster_compression: ClusterCompressionType,

    /// Encryption type for the digest table and all clusters
    pub cluster_encryption: ClusterEncryptionType,

    /// Nonce values for each encrypted cluster
    #[br(count = if cluster_encryption != ClusterEncryptionType::None { cluster_count } else { 0 })]
    pub nonce_table: Vec<[u8; 12]>,

    /// Encryption key for all clusters
    pub cluster_key: [u8; 32],
}

impl std::fmt::Debug for ProtectedHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProtectedHeader")
            .field("block_size", &self.block_size)
            .field("cluster_count", &self.cluster_count)
            .field("cluster_compression", &self.cluster_compression)
            .field("cluster_encryption", &self.cluster_encryption)
            .finish_non_exhaustive()
    }
}

#[derive(BinRead, BinWrite, Debug)]
#[brw(big)]
pub struct Directory {
    /// Nonce value used to encrypt the protected header
    pub protected_nonce: [u8; 12],

    /// Size of the protected header in bytes
    pub protected_size: u32,

    /// Nonce value used to encrypt the digest table
    pub digest_table_nonce: [u8; 12],

    /// Byte offset of the digest table within the image
    pub digest_table_offset: u64,

    /// Size of the digest table in bytes
    pub digest_table_size: u32,
}

/// Mapping of blocks to clusters within the image.
#[derive(BinRead, BinWrite, Debug, Eq, PartialEq, Clone)]
#[brw(big)]
pub struct DigestTable {
    /// Number of digests. This is the same as the number of blocks, but not necessarily
    /// the same as the number of clusters.
    pub digest_count: u32,

    /// Entry for each block
    #[br(count = digest_count)]
    pub digest_table: Vec<DigestTableEntry>,
}

/// An entry in the digest table which corresponds to one cluster.
#[derive(BinRead, BinWrite, Debug, Eq, PartialEq, Clone)]
#[brw(big)]
pub struct DigestTableEntry {
    /// The cluster's offset in the image file
    pub cluster_offset: u64,

    /// Byte offset into the data (blocks)
    pub block_offset: u64,

    /// SHA256 hash of the block before compression and/or encryption were applied
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
    let hash: [u8; 32] = Sha256::new()
        .chain_update(password.as_bytes())
        .finalize()
        .into();
    Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&hash))
}

/// Build a map from `cluster_offset` to the unique-cluster ordinal (the
/// position of that cluster_offset in first-seen order across the digest
/// table). Used to look up per-cluster nonces, since `nonce_table` only
/// holds one entry per *unique* cluster while `digest_table` has one entry
/// per *block* (deduplicated clusters share an offset and thus a nonce).
fn build_cluster_ordinal_map(digest_table: &[DigestTableEntry]) -> HashMap<u64, usize> {
    let mut map: HashMap<u64, usize> = HashMap::new();
    for entry in digest_table {
        let next_idx = map.len();
        map.entry(entry.cluster_offset).or_insert(next_idx);
    }
    map
}

/// GBMF manifest magic. The server's `/manifest` endpoint returns a small
/// binary blob in this format so a streaming client can parse the four
/// metadata sections (primary header, protected header, directory, digest
/// table) without seeking into the much larger cluster region.
pub const MANIFEST_MAGIC: &[u8; 4] = b"GBMF";
pub const MANIFEST_VERSION: u8 = 1;
pub const MANIFEST_FLAG_HEADERS_ENCRYPTED: u8 = 0x01;

/// Parsed manifest in raw byte form: each section is the bytes as they
/// appear on disk (encrypted if the source `.gb` is encrypted). Decrypted
/// metadata is obtained via [`parse_manifest`].
pub struct ManifestBlob {
    pub headers_encrypted: bool,
    pub primary_bytes: Vec<u8>,
    pub protected_bytes: Vec<u8>,
    pub directory_bytes: Vec<u8>,
    pub digest_table_bytes: Vec<u8>,
}

impl ManifestBlob {
    /// Read a manifest blob from a server response or in-memory buffer.
    pub fn read_from<R: Read>(reader: &mut R) -> Result<Self> {
        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic)?;
        if &magic != MANIFEST_MAGIC {
            bail!("invalid manifest magic: {:?}", magic);
        }
        let mut hdr = [0u8; 4];
        reader.read_exact(&mut hdr)?;
        if hdr[0] != MANIFEST_VERSION {
            bail!("unsupported manifest version: {}", hdr[0]);
        }
        let headers_encrypted = hdr[1] & MANIFEST_FLAG_HEADERS_ENCRYPTED != 0;

        let primary_bytes = read_length_prefixed(reader)?;
        let protected_bytes = read_length_prefixed(reader)?;
        let directory_bytes = read_length_prefixed(reader)?;
        let digest_table_bytes = read_length_prefixed(reader)?;
        Ok(Self {
            headers_encrypted,
            primary_bytes,
            protected_bytes,
            directory_bytes,
            digest_table_bytes,
        })
    }

    /// Serialise the manifest blob to bytes.
    pub fn write_to(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(MANIFEST_MAGIC);
        out.push(MANIFEST_VERSION);
        out.push(if self.headers_encrypted {
            MANIFEST_FLAG_HEADERS_ENCRYPTED
        } else {
            0
        });
        out.extend_from_slice(&[0u8; 2]); // reserved
        write_length_prefixed(&mut out, &self.primary_bytes);
        write_length_prefixed(&mut out, &self.protected_bytes);
        write_length_prefixed(&mut out, &self.directory_bytes);
        write_length_prefixed(&mut out, &self.digest_table_bytes);
        out
    }
}

fn read_length_prefixed<R: Read>(reader: &mut R) -> Result<Vec<u8>> {
    let mut len_bytes = [0u8; 4];
    reader.read_exact(&mut len_bytes)?;
    let len = u32::from_be_bytes(len_bytes) as usize;
    if len > 32 * 1024 * 1024 {
        bail!("manifest section length {} exceeds 32 MiB cap", len);
    }
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf)?;
    Ok(buf)
}

fn write_length_prefixed(out: &mut Vec<u8>, data: &[u8]) {
    let len = data.len() as u32;
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(data);
}

/// Parse a manifest blob into the four typed metadata structures, decrypting
/// each section if the source image is encrypted. Returns the cluster region
/// start offset (= end of protected header bytes in the original `.gb` file).
pub fn parse_manifest(
    blob: &ManifestBlob,
    password: Option<String>,
) -> Result<(PrimaryHeader, ProtectedHeader, Directory, DigestTable, u64)> {
    let primary: PrimaryHeader = Cursor::new(&blob.primary_bytes).read_be()?;

    let header_cipher = new_key(password.unwrap_or_default());
    let directory: Directory = match primary.encryption_type {
        HeaderEncryptionType::None => Cursor::new(&blob.directory_bytes).read_be()?,
        HeaderEncryptionType::Aes256 => {
            let plain = header_cipher
                .decrypt(
                    Nonce::from_slice(&primary.directory_nonce),
                    blob.directory_bytes.as_slice(),
                )
                .map_err(|e| anyhow::anyhow!("decrypt directory: {e}"))?;
            Cursor::new(plain).read_be()?
        }
    };

    let protected: ProtectedHeader = match primary.encryption_type {
        HeaderEncryptionType::None => Cursor::new(&blob.protected_bytes).read_be()?,
        HeaderEncryptionType::Aes256 => {
            let plain = header_cipher
                .decrypt(
                    Nonce::from_slice(&directory.protected_nonce),
                    blob.protected_bytes.as_slice(),
                )
                .map_err(|e| anyhow::anyhow!("decrypt protected: {e}"))?;
            Cursor::new(plain).read_be()?
        }
    };

    let digest: DigestTable = match primary.encryption_type {
        HeaderEncryptionType::None => Cursor::new(&blob.digest_table_bytes).read_be()?,
        HeaderEncryptionType::Aes256 => {
            let plain = header_cipher
                .decrypt(
                    Nonce::from_slice(&directory.digest_table_nonce),
                    blob.digest_table_bytes.as_slice(),
                )
                .map_err(|e| anyhow::anyhow!("decrypt digest_table: {e}"))?;
            Cursor::new(plain).read_be()?
        }
    };

    let cluster_region_start = blob.primary_bytes.len() as u64 + blob.protected_bytes.len() as u64;
    Ok((primary, protected, directory, digest, cluster_region_start))
}

/// Hash the cluster region of an image file (between the protected header
/// and the digest table) to produce the content ID.
pub fn compute_content_id(
    path: impl AsRef<Path>,
    cluster_region_start: u64,
    cluster_region_end: u64,
) -> Result<[u8; 32]> {
    use sha2::Digest;
    let mut file = File::open(&path)?;
    file.seek(SeekFrom::Start(cluster_region_start))?;
    let mut remaining = cluster_region_end - cluster_region_start;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65536];
    while remaining > 0 {
        let take = remaining.min(buf.len() as u64) as usize;
        let n = file.read(&mut buf[..take])?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        remaining -= n as u64;
    }
    Ok(hasher.finalize().into())
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

        // The image's "id" is now the content ID (cluster-region SHA256),
        // already stored in the primary header.
        let id = primary_header.content_id_hex();

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
                file_size: std::fs::metadata(path)?.len(),
            })
        } else {
            Ok(Self {
                id,
                primary_header,
                protected_header: None,
                digest_table: None,
                directory: None,
                path: path.to_path_buf(),
                file_size: std::fs::metadata(path)?.len(),
            })
        }
    }

    /// Compute the byte length of the primary header as serialised on
    /// disk. Used by the registry server (and other tools) to slice .gb
    /// files at known offsets without re-implementing the binary layout.
    pub fn primary_header_len(&self) -> Result<u64> {
        let mut cur = Cursor::new(Vec::new());
        self.primary_header.write(&mut cur)?;
        Ok(cur.into_inner().len() as u64)
    }

    /// Read a [`ManifestBlob`] directly from this image's on-disk file. The
    /// returned bytes are exactly the four metadata sections as stored
    /// (still encrypted if the source is encrypted). Used by the registry
    /// server to answer `/manifest` requests without ever needing the
    /// image password.
    pub fn read_manifest_blob(&self) -> Result<ManifestBlob> {
        let directory = self
            .directory
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("directory not loaded"))?;
        let primary_len = self.primary_header_len()?;

        let mut file = File::open(&self.path)?;
        let mut primary_bytes = vec![0u8; primary_len as usize];
        file.seek(SeekFrom::Start(0))?;
        file.read_exact(&mut primary_bytes)?;

        let mut protected_bytes = vec![0u8; directory.protected_size as usize];
        file.seek(SeekFrom::Start(primary_len))?;
        file.read_exact(&mut protected_bytes)?;

        let mut digest_table_bytes = vec![0u8; directory.digest_table_size as usize];
        file.seek(SeekFrom::Start(directory.digest_table_offset))?;
        file.read_exact(&mut digest_table_bytes)?;

        let mut directory_bytes = vec![0u8; self.primary_header.directory_size as usize];
        file.seek(SeekFrom::Start(self.primary_header.directory_offset))?;
        file.read_exact(&mut directory_bytes)?;

        Ok(ManifestBlob {
            headers_encrypted: matches!(
                self.primary_header.encryption_type,
                HeaderEncryptionType::Aes256
            ),
            primary_bytes,
            protected_bytes,
            directory_bytes,
            digest_table_bytes,
        })
    }

    /// Compute the byte range `[start, end)` within the underlying `.gb`
    /// file that contains the cluster region (i.e. all `Cluster` records
    /// laid out back-to-back, excluding the headers and the trailing digest
    /// table / directory). Used by the registry server to stream the
    /// cluster region in response to `/clusters` requests without parsing
    /// any encrypted bytes itself.
    ///
    /// Requires that `directory` and `protected_header.cluster_count > 0`
    /// have been loaded — typically via [`ImageHandle::open`] (unencrypted)
    /// or [`ImageHandle::load`] (encrypted).
    pub fn cluster_region_bounds(&self) -> Result<(u64, u64)> {
        let directory = self
            .directory
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("directory not loaded"))?;

        // The protected header sits immediately after the primary header.
        let mut primary_only = Cursor::new(Vec::new());
        self.primary_header.write(&mut primary_only)?;
        let primary_len = primary_only.into_inner().len() as u64;
        let start = primary_len + directory.protected_size as u64;
        let end = directory.digest_table_offset;
        if end < start {
            bail!(
                "malformed image: digest_table_offset {} precedes cluster region start {}",
                end,
                start
            );
        }
        Ok((start, end))
    }

    /// Modify the password and re-encrypt all encrypted sections. This doesn't
    /// re-encrypt the clusters because they are encrypted with the cluster key.
    pub fn change_password(&self, _old_password: String, new_password: String) -> Result<()> {
        // Create the cipher and a RNG for the nonces
        let _cipher = new_key(new_password);

        todo!()
    }

    /// Write the image contents out to disk.
    ///
    /// The progress callback receives `(cluster_index, state)` for each cluster:
    /// - `None`        — cluster is dirty and is now being written
    /// - `Some(true)`  — cluster was dirty and has been written
    /// - `Some(false)` — cluster was already up to date, no write needed
    pub fn write<F: Fn(usize, Option<bool>)>(
        &self,
        dest: impl AsRef<Path>,
        preload: bool,
        progress: F,
    ) -> Result<()> {
        if self.protected_header.is_none() || self.digest_table.is_none() {
            bail!("Image not loaded");
        }

        let protected_header = self.protected_header.clone().unwrap();
        let digest_table = self.digest_table.clone().unwrap().digest_table;

        let cluster_cipher =
            Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&protected_header.cluster_key));

        let dest = dest.as_ref();
        info!(image = ?self, dest = ?dest, "Preparing to write image");

        let mut dest = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .read(true)
            .open(dest)?;

        // Open the cluster table for reading, optionally loading the entire image
        // into memory for faster access.
        let mut cluster_table: Box<dyn ReadSeek> = if preload {
            debug!(
                file_size = self.file_size,
                "Loading entire image into memory"
            );
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

        // Map cluster_offset → unique-cluster ordinal (nonce_table index)
        let cluster_ordinal = build_cluster_ordinal_map(&digest_table);

        // Write all of the clusters that have changed
        for (i, entry) in digest_table.iter().enumerate() {
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

                // Reverse encryption — nonces are keyed by unique-cluster ordinal,
                // not by digest-table index (multiple digest entries can alias the
                // same cluster_offset via dedup).
                cluster.data = match protected_header.cluster_encryption {
                    ClusterEncryptionType::None => cluster.data,
                    ClusterEncryptionType::Aes256 => {
                        let nonce_idx = *cluster_ordinal
                            .get(&entry.cluster_offset)
                            .ok_or_else(|| anyhow::anyhow!("missing cluster ordinal"))?;
                        cluster_cipher
                            .decrypt(
                                Nonce::from_slice(&protected_header.nonce_table[nonce_idx]),
                                cluster.data.as_ref(),
                            )
                            .map_err(|e| anyhow::anyhow!("decryption failed: {e}"))?
                    }
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

        Ok(())
    }

    /// Write image contents to `dest` by consuming a *stream* of cluster
    /// bytes (no local seekable copy of the `.gb` file required).
    ///
    /// `cluster_stream` must deliver the byte range
    /// `[cluster_data_start_offset .. digest_table_offset)` from the source
    /// `.gb` file in order. Clusters in this range are encoded as
    /// `Cluster { size: u32 BE, data: [u8; size] }` back-to-back, in the
    /// same order they appear in the file (which matches the first-seen
    /// order of unique `cluster_offset`s in the digest table — clusters are
    /// always appended monotonically by `from_qcow`).
    ///
    /// The function tolerates duplicate digest entries pointing at the same
    /// `cluster_offset`: each cluster is consumed from the stream exactly
    /// once, decoded, then placed at every `block_offset` that references
    /// it. Per-block hashing avoids unnecessary writes.
    pub fn stream_write<R: Read, F: Fn(usize, Option<bool>)>(
        primary_header: &PrimaryHeader,
        protected_header: &ProtectedHeader,
        digest_table: &DigestTable,
        cluster_stream: R,
        cluster_data_start_offset: u64,
        dest: impl AsRef<Path>,
        progress: F,
    ) -> Result<()> {
        let cluster_cipher =
            Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&protected_header.cluster_key));

        // Build (unique_idx, cluster_offset) in first-seen order across the
        // digest table, plus the reverse index for placement at write time.
        let mut unique_offsets: Vec<u64> = Vec::new();
        let mut nonce_idx_by_offset: HashMap<u64, usize> = HashMap::new();
        let mut entries_by_offset: HashMap<u64, Vec<usize>> = HashMap::new();
        for (i, entry) in digest_table.digest_table.iter().enumerate() {
            if let std::collections::hash_map::Entry::Vacant(e) =
                nonce_idx_by_offset.entry(entry.cluster_offset)
            {
                e.insert(unique_offsets.len());
                unique_offsets.push(entry.cluster_offset);
            }
            entries_by_offset
                .entry(entry.cluster_offset)
                .or_default()
                .push(i);
        }

        // Sort streaming order ascending — this matches `from_qcow`'s output
        // (clusters are appended monotonically), but we sort defensively to
        // catch any future format where first-seen order would diverge from
        // file order. If they ever differ, the stream-skip step below will
        // hit a negative delta and we'll bail loudly.
        unique_offsets.sort_unstable();

        let dest = dest.as_ref();
        info!(dest = ?dest, "Preparing to stream-write image");

        let mut dest_file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .read(true)
            .open(dest)?;

        // Extend regular files to the full virtual disk size. Do not
        // truncate — when writing to a block device, the file represents the
        // whole device and truncate() would be either no-op or harmful.
        let meta = dest_file.metadata()?;
        if meta.is_file() && meta.len() < primary_header.size {
            dest_file.set_len(primary_header.size)?;
        }

        let mut block = vec![0u8; protected_header.block_size as usize];
        let mut reader = BufReader::new(cluster_stream);
        let mut stream_pos = cluster_data_start_offset;
        let max_cluster_bytes = (protected_header.block_size as u64).saturating_mul(4);

        for cluster_offset in unique_offsets {
            // Advance the stream to the cluster's offset by reading and
            // discarding any padding bytes. Negative delta = fatal.
            if cluster_offset < stream_pos {
                bail!(
                    "stream delivered clusters out of order: pos={} but next cluster_offset={}",
                    stream_pos,
                    cluster_offset
                );
            }
            let skip = cluster_offset - stream_pos;
            if skip > 0 {
                std::io::copy(&mut (&mut reader).take(skip), &mut std::io::sink())?;
            }
            stream_pos = cluster_offset;

            // Read the cluster header (u32 BE size)
            let mut size_bytes = [0u8; 4];
            reader.read_exact(&mut size_bytes)?;
            let cluster_size = u32::from_be_bytes(size_bytes) as u64;
            if cluster_size > max_cluster_bytes {
                bail!(
                    "cluster at offset {} declares size {} which exceeds the {}-byte cap",
                    cluster_offset,
                    cluster_size,
                    max_cluster_bytes
                );
            }
            let mut data = Vec::with_capacity(cluster_size as usize);
            (&mut reader).take(cluster_size).read_to_end(&mut data)?;
            if data.len() as u64 != cluster_size {
                bail!(
                    "short read on cluster at offset {}: expected {}, got {}",
                    cluster_offset,
                    cluster_size,
                    data.len()
                );
            }
            stream_pos += 4 + cluster_size;

            // Reverse encryption
            let nonce_idx = *nonce_idx_by_offset
                .get(&cluster_offset)
                .expect("nonce idx for known cluster_offset");
            let plain = match protected_header.cluster_encryption {
                ClusterEncryptionType::None => data,
                ClusterEncryptionType::Aes256 => cluster_cipher
                    .decrypt(
                        Nonce::from_slice(&protected_header.nonce_table[nonce_idx]),
                        data.as_ref(),
                    )
                    .map_err(|e| anyhow::anyhow!("decrypt cluster {cluster_offset}: {e}"))?,
            };

            // Reverse compression
            let plain = match protected_header.cluster_compression {
                ClusterCompressionType::None => plain,
                ClusterCompressionType::Zstd => zstd::decode_all(Cursor::new(plain))?,
            };

            // Place at every block_offset that references this cluster.
            let entry_indices = entries_by_offset
                .get(&cluster_offset)
                .expect("entries for known cluster_offset");
            for &i in entry_indices {
                let entry = &digest_table.digest_table[i];

                dest_file.seek(SeekFrom::Start(entry.block_offset))?;
                let hash: [u8; 32] = match dest_file.read_exact(&mut block) {
                    Ok(_) => Sha256::new().chain_update(&block).finalize().into(),
                    Err(_) => [0u8; 32],
                };
                let is_dirty = hash != entry.digest;

                if is_dirty {
                    progress(i, None);
                    dest_file.seek(SeekFrom::Start(entry.block_offset))?;

                    // Handle a trailing partial block at end-of-disk
                    let max_len = primary_header.size.saturating_sub(entry.block_offset) as usize;
                    let to_write = if plain.len() > max_len {
                        &plain[..max_len]
                    } else {
                        &plain[..]
                    };
                    dest_file.write_all(to_write)?;
                }

                progress(i, Some(is_dirty));
            }
        }

        Ok(())
    }

    /// Verify the image contents on disk by reading and hashing each block.
    ///
    /// The progress callback receives `(cluster_index, verified)`:
    /// - `None`       — cluster is now being read/hashed
    /// - `Some(true)` — cluster hash matched
    /// - `Some(false)`— cluster hash did not match (corruption detected)
    pub fn verify<F: Fn(usize, Option<bool>)>(
        &self,
        dest: impl AsRef<Path>,
        progress: F,
    ) -> Result<()> {
        if self.protected_header.is_none() || self.digest_table.is_none() {
            bail!("Image not loaded");
        }

        let protected_header = self.protected_header.clone().unwrap();
        let digest_table = self.digest_table.clone().unwrap().digest_table;

        let mut dest = std::fs::OpenOptions::new().read(true).open(dest)?;

        let mut block = vec![0u8; protected_header.block_size as usize];

        for (i, entry) in digest_table.iter().enumerate() {
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
    ///
    /// `name` and `tag` are validated and written into the image's
    /// `PrimaryHeader`. They participate in the on-disk header bytes and
    /// therefore in any later whole-file integrity checks, but are
    /// deliberately not part of `content_id` — that is computed over the
    /// cluster region only.
    pub fn from_qcow<F: Fn(u64, u64)>(
        name: &str,
        tag: &str,
        metadata: Vec<ElementHeader>,
        source: &Qcow3,
        dest: impl AsRef<Path>,
        password: Option<String>,
        progress: F,
    ) -> Result<ImageHandle> {
        info!(qcow = ?source, "Converting qcow image to goldboot image");

        validate_ref_segment(name).context("invalid image name")?;
        validate_ref_segment(tag).context("invalid image tag")?;

        let mut dest_file = File::create(&dest)?;
        let mut source_file = File::open(&source.path)?;

        // Prepare cipher and RNG if the image header should be encrypted
        let header_cipher = new_key(password.clone().unwrap_or("".to_string()));
        let mut rng = rand::rng();

        // Prepare directory
        let mut directory = Directory {
            protected_nonce: {
                let mut b = [0u8; 12];
                rng.fill_bytes(&mut b);
                b
            },
            protected_size: 0,
            digest_table_nonce: {
                let mut b = [0u8; 12];
                rng.fill_bytes(&mut b);
                b
            },
            digest_table_offset: 0,
            digest_table_size: 0,
        };

        let name_bytes = name.as_bytes().to_vec();
        let tag_bytes = tag.as_bytes().to_vec();

        // Prepare primary header (content_id starts as zeros and is patched
        // in after the cluster region is written).
        let mut primary_header = PrimaryHeader {
            version: 2,
            arch: ImageArch::Amd64,   // TODO
            size: source.header.size, // TODO this is aligned to the cluster size?
            directory_nonce: {
                let mut b = [0u8; 12];
                rng.fill_bytes(&mut b);
                b
            },
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
            name_length: u8::try_from(name_bytes.len()).context("name too long")?,
            name: name_bytes,
            tag_length: u8::try_from(tag_bytes.len()).context("tag too long")?,
            tag: tag_bytes,
            content_id: [0u8; 32],
        };

        // Prepare protected header
        let cluster_count = source.count_clusters()? as u32;
        let mut protected_header = ProtectedHeader {
            block_size: source.header.cluster_size() as u32,
            cluster_count,
            cluster_compression: ClusterCompressionType::Zstd,
            cluster_encryption: if password.is_some() {
                ClusterEncryptionType::Aes256
            } else {
                ClusterEncryptionType::None
            },
            cluster_key: {
                let mut b = [0u8; 32];
                rng.fill_bytes(&mut b);
                b
            },
            nonce_table: if password.is_some() {
                (0..cluster_count)
                    .map(|_| {
                        let mut b = [0u8; 12];
                        rng.fill_bytes(&mut b);
                        b
                    })
                    .collect()
            } else {
                vec![]
            },
        };

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
        let mut i = 0;

        // Track the cluster offset in the image file
        let mut cluster_offset = dest_file.stream_position()?;

        // Streaming hasher over the cluster region. The on-disk layout of
        // each cluster is `size: u32 BE` followed by `data: [u8; size]`, so
        // we feed exactly those bytes — which is what `Cluster::write` emits.
        let mut content_hasher = Sha256::new();

        // Read from the qcow2 and write the clusters
        for l1_entry in &source.l1_table {
            if let Some(l2_table) = l1_entry.read_l2(&mut source_file, source.header.cluster_bits) {
                for l2_entry in l2_table {
                    if let Some(contents) = l2_entry.read_contents(
                        &mut source_file,
                        source.header.cluster_size(),
                        source.header.compression_type,
                    )? {
                        // Start building the cluster
                        let mut cluster = Cluster {
                            // We don't know the final size until we compress/encrypt
                            size: 0,
                            data: contents,
                        };

                        // Truncate the final cluster if the disk size is not cluster-aligned
                        if block_offset + source.header.cluster_size() > primary_header.size {
                            cluster
                                .data
                                .truncate((primary_header.size - block_offset) as usize);
                        }

                        // Compute hash of the block which will be used when writing the block later
                        let digest = Sha256::new().chain_update(&cluster.data).finalize();

                        // If the hash already exists, skip writing the cluster. TODO: faster data structure
                        let mut existing_cluster_offset = None;
                        for entry in digest_table.digest_table.iter() {
                            if entry.digest == *digest {
                                existing_cluster_offset = Some(entry.cluster_offset);
                                break;
                            }
                        }

                        digest_table.digest_table.push(DigestTableEntry {
                            digest: digest.into(),
                            block_offset,
                            cluster_offset: existing_cluster_offset.unwrap_or(cluster_offset),
                        });

                        if existing_cluster_offset.is_none() {
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
                                        Nonce::from_slice(&protected_header.nonce_table[i]),
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
                            content_hasher.update(cluster.size.to_be_bytes());
                            content_hasher.update(&cluster.data);

                            // Advance offset
                            cluster_offset += 4; // size field (u32)
                            cluster_offset += cluster.size as u64;
                            i += 1;
                        } else {
                            // Discount a cluster and a nonce because we didn't write the cluster
                            protected_header.cluster_count -= 1;
                            protected_header.nonce_table.pop();
                        }
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

        // Finalize the content_id now that the entire cluster region has
        // been written.
        primary_header.content_id = content_hasher.finalize().into();

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
            id: primary_header.content_id_hex(),
            primary_header,
            protected_header: Some(protected_header),
            digest_table: Some(digest_table),
            directory: Some(directory),
            file_size: std::fs::metadata(&dest)?.len(),
            path: dest.as_ref().to_path_buf(),
        })
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Test helpers
// ────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use rand::Rng;

    /// Build a synthetic .gb image from a list of raw blocks, deduplicating by
    /// content hash so duplicate blocks share a cluster (and thus a nonce).
    /// Encryption + zstd compression always on. Used to deterministically
    /// exercise the dedup/nonce code paths without depending on qemu-img.
    pub(crate) fn build_synthetic_image(
        path: &Path,
        blocks: &[Vec<u8>],
        block_size: u32,
        password: &str,
    ) -> Result<()> {
        let header_cipher = new_key(password.to_string());
        let mut rng = rand::rng();

        // Compute digests and figure out unique clusters (first-seen order)
        let mut unique_block_data: Vec<Vec<u8>> = Vec::new();
        let mut digest_to_unique_idx: HashMap<[u8; 32], usize> = HashMap::new();
        let mut per_entry: Vec<(u64, [u8; 32], usize)> = Vec::new(); // (block_offset, digest, unique_idx)
        let mut block_offset: u64 = 0;
        for block in blocks {
            let digest: [u8; 32] = Sha256::new().chain_update(block).finalize().into();
            let unique_idx = match digest_to_unique_idx.get(&digest) {
                Some(idx) => *idx,
                None => {
                    let idx = unique_block_data.len();
                    digest_to_unique_idx.insert(digest, idx);
                    unique_block_data.push(block.clone());
                    idx
                }
            };
            per_entry.push((block_offset, digest, unique_idx));
            block_offset += block_size as u64;
        }
        let unique_cluster_count = unique_block_data.len();
        let total_size: u64 = blocks.iter().map(|b| b.len() as u64).sum();

        // Build per-cluster nonces and the cluster encryption key
        let nonce_table: Vec<[u8; 12]> = (0..unique_cluster_count)
            .map(|_| {
                let mut b = [0u8; 12];
                rng.fill_bytes(&mut b);
                b
            })
            .collect();
        let mut cluster_key = [0u8; 32];
        rng.fill_bytes(&mut cluster_key);
        let cluster_cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&cluster_key));

        let protected_header = ProtectedHeader {
            block_size,
            cluster_count: unique_cluster_count as u32,
            cluster_compression: ClusterCompressionType::Zstd,
            cluster_encryption: ClusterEncryptionType::Aes256,
            nonce_table: nonce_table.clone(),
            cluster_key,
        };

        // Encode the protected header (will be encrypted)
        let mut protected_bytes = Cursor::new(Vec::new());
        protected_header.write(&mut protected_bytes)?;
        let protected_nonce = {
            let mut b = [0u8; 12];
            rng.fill_bytes(&mut b);
            b
        };
        let protected_enc = header_cipher
            .encrypt(
                Nonce::from_slice(&protected_nonce),
                protected_bytes.into_inner().as_slice(),
            )
            .map_err(|e| anyhow::anyhow!("encrypt protected: {e}"))?;

        let directory_nonce = {
            let mut b = [0u8; 12];
            rng.fill_bytes(&mut b);
            b
        };
        let digest_table_nonce = {
            let mut b = [0u8; 12];
            rng.fill_bytes(&mut b);
            b
        };

        // Build a placeholder primary header so we can compute its serialised
        // length (the cluster region starts immediately after).
        let mut primary_header = PrimaryHeader {
            version: 2,
            size: total_size,
            timestamp: 0,
            encryption_type: HeaderEncryptionType::Aes256,
            element_count: 1,
            elements: vec![ElementHeader::new("test", "synthetic")?],
            arch: ImageArch::Amd64,
            name_length: 9,
            name: b"synthetic".to_vec(),
            tag_length: 4,
            tag: b"test".to_vec(),
            content_id: [0u8; 32],
            directory_nonce,
            directory_offset: 0,
            directory_size: 0,
        };

        // Write the file
        let mut dest = File::create(path)?;
        primary_header.write(&mut dest)?;
        dest.write_all(&protected_enc)?;

        // Write clusters, recording cluster_offsets per unique cluster, and
        // stream-hash them to populate content_id.
        let mut cluster_offsets: Vec<u64> = Vec::with_capacity(unique_cluster_count);
        let mut content_hasher = Sha256::new();
        for (i, block_data) in unique_block_data.iter().enumerate() {
            let off = dest.stream_position()?;
            cluster_offsets.push(off);

            let compressed = zstd::encode_all(Cursor::new(block_data.as_slice()), 0)?;
            let encrypted = cluster_cipher
                .encrypt(Nonce::from_slice(&nonce_table[i]), compressed.as_slice())
                .map_err(|e| anyhow::anyhow!("encrypt cluster: {e}"))?;
            let cluster = Cluster {
                size: encrypted.len() as u32,
                data: encrypted,
            };
            cluster.write(&mut dest)?;
            content_hasher.update(cluster.size.to_be_bytes());
            content_hasher.update(&cluster.data);
        }
        primary_header.content_id = content_hasher.finalize().into();

        // Build digest_table
        let digest_table = DigestTable {
            digest_count: per_entry.len() as u32,
            digest_table: per_entry
                .iter()
                .map(|(block_offset, digest, unique_idx)| DigestTableEntry {
                    cluster_offset: cluster_offsets[*unique_idx],
                    block_offset: *block_offset,
                    digest: *digest,
                })
                .collect(),
        };
        let mut digest_bytes = Cursor::new(Vec::new());
        digest_table.write(&mut digest_bytes)?;
        let digest_enc = header_cipher
            .encrypt(
                Nonce::from_slice(&digest_table_nonce),
                digest_bytes.into_inner().as_slice(),
            )
            .map_err(|e| anyhow::anyhow!("encrypt digest_table: {e}"))?;

        let digest_table_offset = dest.stream_position()?;
        dest.write_all(&digest_enc)?;

        // Build and write directory
        let directory = Directory {
            protected_nonce,
            protected_size: protected_enc.len() as u32,
            digest_table_nonce,
            digest_table_offset,
            digest_table_size: digest_enc.len() as u32,
        };
        let mut dir_bytes = Cursor::new(Vec::new());
        directory.write(&mut dir_bytes)?;
        let dir_enc = header_cipher
            .encrypt(
                Nonce::from_slice(&directory_nonce),
                dir_bytes.into_inner().as_slice(),
            )
            .map_err(|e| anyhow::anyhow!("encrypt directory: {e}"))?;

        let directory_offset = dest.stream_position()?;
        dest.write_all(&dir_enc)?;

        // Patch the primary header's directory_offset/size
        primary_header.directory_offset = directory_offset;
        primary_header.directory_size = dir_enc.len() as u32;
        dest.seek(SeekFrom::Start(0))?;
        primary_header.write(&mut dest)?;
        dest.flush()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::build_synthetic_image;
    use super::*;
    use tempfile::tempdir;

    /// Regression test for the nonce_table indexing bug: an encrypted image
    /// containing duplicate blocks must round-trip through write() without
    /// panicking or producing incorrect bytes.
    #[test]
    fn write_encrypted_with_duplicate_clusters() -> Result<()> {
        let dir = tempdir()?;
        let img_path = dir.path().join("image.gb");
        let dest_path = dir.path().join("disk.raw");

        let block_size: u32 = 4096;
        let block_a = vec![0xAAu8; block_size as usize];
        let block_b = vec![0xBBu8; block_size as usize];
        let blocks = vec![block_a.clone(), block_b.clone(), block_a.clone()];

        build_synthetic_image(&img_path, &blocks, block_size, "test")?;

        let mut handle = ImageHandle::open(&img_path)?;
        handle.load(Some("test".to_string()))?;
        handle.write(&dest_path, false, |_, _| {})?;

        let mut actual = Vec::new();
        File::open(&dest_path)?.read_to_end(&mut actual)?;

        let mut expected = Vec::new();
        for b in &blocks {
            expected.extend_from_slice(b);
        }
        assert_eq!(
            actual, expected,
            "round-tripped disk image must match input"
        );
        Ok(())
    }

    /// stream_write must correctly reconstruct a disk by consuming the
    /// cluster region as a Read stream. Exercises dedup, encryption, and
    /// the GBMF manifest round-trip.
    #[test]
    fn stream_write_round_trip_encrypted_with_duplicates() -> Result<()> {
        let dir = tempdir()?;
        let img_path = dir.path().join("image.gb");
        let dest_path = dir.path().join("disk.raw");

        let block_size: u32 = 4096;
        let block_a = vec![0x11u8; block_size as usize];
        let block_b = vec![0x22u8; block_size as usize];
        let blocks = vec![
            block_a.clone(),
            block_b.clone(),
            block_a.clone(),
            block_b.clone(),
            block_a.clone(),
        ];

        build_synthetic_image(&img_path, &blocks, block_size, "pw")?;

        // Load the image normally so we have the parsed headers
        let mut handle = ImageHandle::open(&img_path)?;
        handle.load(Some("pw".to_string()))?;
        let primary = handle.primary_header;
        let protected = handle.protected_header.unwrap();
        let directory = handle.directory.unwrap();
        let digest = handle.digest_table.unwrap();

        // Compute cluster region bounds from the parsed headers
        let mut primary_only = Cursor::new(Vec::new());
        primary.write(&mut primary_only)?;
        let primary_len = primary_only.into_inner().len() as u64;
        let cluster_start = primary_len + directory.protected_size as u64;
        let cluster_end = directory.digest_table_offset;

        // Slice out the cluster region as the "network stream"
        let mut img = File::open(&img_path)?;
        img.seek(SeekFrom::Start(cluster_start))?;
        let mut cluster_region = vec![0u8; (cluster_end - cluster_start) as usize];
        img.read_exact(&mut cluster_region)?;

        ImageHandle::stream_write(
            &primary,
            &protected,
            &digest,
            Cursor::new(cluster_region),
            cluster_start,
            &dest_path,
            |_, _| {},
        )?;

        let mut actual = Vec::new();
        File::open(&dest_path)?.read_to_end(&mut actual)?;
        let mut expected = Vec::new();
        for b in &blocks {
            expected.extend_from_slice(b);
        }
        assert_eq!(actual, expected, "stream-written disk must match input");
        Ok(())
    }

    /// stream_write must fail cleanly on a bogus oversized cluster size
    /// (cap protects against allocation DoS from a malicious server).
    #[test]
    fn stream_write_rejects_oversized_cluster() -> Result<()> {
        let dir = tempdir()?;
        let img_path = dir.path().join("image.gb");
        let block_size: u32 = 4096;
        let blocks = vec![vec![0x33u8; block_size as usize]];
        build_synthetic_image(&img_path, &blocks, block_size, "pw")?;

        let mut handle = ImageHandle::open(&img_path)?;
        handle.load(Some("pw".to_string()))?;
        let primary = handle.primary_header;
        let protected = handle.protected_header.unwrap();
        let directory = handle.directory.unwrap();
        let digest = handle.digest_table.unwrap();

        let mut primary_only = Cursor::new(Vec::new());
        primary.write(&mut primary_only)?;
        let primary_len = primary_only.into_inner().len() as u64;
        let cluster_start = primary_len + directory.protected_size as u64;

        // Craft a malicious "stream" that announces a 1 GiB cluster — way
        // above the 4 * block_size cap.
        let mut evil = Vec::new();
        evil.extend_from_slice(&(1024u32 * 1024 * 1024).to_be_bytes());
        evil.extend_from_slice(&[0u8; 8]);

        let dest_path = dir.path().join("disk.raw");
        let result = ImageHandle::stream_write(
            &primary,
            &protected,
            &digest,
            Cursor::new(evil),
            cluster_start,
            &dest_path,
            |_, _| {},
        );
        assert!(
            result.is_err(),
            "stream_write must reject oversized cluster size"
        );
        Ok(())
    }

    #[test]
    fn image_ref_parse_bare_name() {
        let r = ImageRef::parse("archlinux").unwrap();
        assert_eq!(r.host, None);
        assert_eq!(r.name, "archlinux");
        assert_eq!(r.tag, None);
        assert_eq!(r.to_string(), "archlinux");
    }

    #[test]
    fn image_ref_parse_name_tag() {
        let r = ImageRef::parse("archlinux:v1").unwrap();
        assert_eq!(r.host, None);
        assert_eq!(r.name, "archlinux");
        assert_eq!(r.tag.as_deref(), Some("v1"));
        assert_eq!(r.to_string(), "archlinux:v1");
    }

    #[test]
    fn image_ref_parse_host_name() {
        let r = ImageRef::parse("registry.example.com/archlinux").unwrap();
        assert_eq!(r.host.as_deref(), Some("registry.example.com"));
        assert_eq!(r.name, "archlinux");
        assert_eq!(r.tag, None);
        assert_eq!(r.to_string(), "registry.example.com/archlinux");
    }

    #[test]
    fn image_ref_parse_host_name_tag() {
        let r = ImageRef::parse("registry.example.com/archlinux:v1").unwrap();
        assert_eq!(r.host.as_deref(), Some("registry.example.com"));
        assert_eq!(r.name, "archlinux");
        assert_eq!(r.tag.as_deref(), Some("v1"));
        assert_eq!(r.to_string(), "registry.example.com/archlinux:v1");
    }

    #[test]
    fn image_ref_parse_host_port() {
        let r = ImageRef::parse("localhost:8080/archlinux:v1").unwrap();
        assert_eq!(r.host.as_deref(), Some("localhost:8080"));
        assert_eq!(r.name, "archlinux");
        assert_eq!(r.tag.as_deref(), Some("v1"));
        assert_eq!(r.host_bare(), Some("localhost:8080"));
    }

    #[test]
    fn image_ref_parse_host_port_no_tag() {
        let r = ImageRef::parse("localhost:8080/archlinux").unwrap();
        assert_eq!(r.host.as_deref(), Some("localhost:8080"));
        assert_eq!(r.name, "archlinux");
        assert_eq!(r.tag, None);
    }

    #[test]
    fn image_ref_parse_scheme_attached_to_host() {
        let r = ImageRef::parse("https://reg.example.com/archlinux:v1").unwrap();
        assert_eq!(r.host.as_deref(), Some("https://reg.example.com"));
        assert_eq!(r.host_bare(), Some("reg.example.com"));
        assert_eq!(r.name, "archlinux");
        assert_eq!(r.tag.as_deref(), Some("v1"));
    }

    #[test]
    fn image_ref_parse_scheme_without_host_rejected() {
        assert!(ImageRef::parse("https://archlinux").is_err());
    }

    #[test]
    fn image_ref_validate_accepts_well_formed() {
        ImageRef::parse("registry.example.com/archlinux:v1.2.3")
            .unwrap()
            .validate()
            .unwrap();
    }

    #[test]
    fn image_ref_validate_rejects_bad_name() {
        let r = ImageRef::parse("registry.example.com/arch linux").unwrap();
        assert!(r.validate().is_err());
    }

    /// `PrimaryHeader.content_id` (populated during build) must match the
    /// independently-recomputed SHA256 over the cluster region bytes. This
    /// is the invariant the registry server relies on: it can trust the
    /// header without re-hashing every push.
    #[test]
    fn content_id_matches_cluster_region_hash() -> Result<()> {
        let dir = tempdir()?;
        let img_path = dir.path().join("image.gb");
        let block_size: u32 = 4096;
        let blocks = vec![
            vec![0xEEu8; block_size as usize],
            vec![0xFFu8; block_size as usize],
        ];
        build_synthetic_image(&img_path, &blocks, block_size, "pw")?;

        let mut handle = ImageHandle::open(&img_path)?;
        handle.load(Some("pw".to_string()))?;
        let (start, end) = handle.cluster_region_bounds()?;

        let recomputed = compute_content_id(&img_path, start, end)?;
        assert_eq!(
            recomputed, handle.primary_header.content_id,
            "header.content_id must equal SHA256 over the cluster region"
        );
        assert_ne!(recomputed, [0u8; 32], "content_id must not be all zeros");
        Ok(())
    }

    /// verify() must report all blocks as matching after a fresh write.
    #[test]
    fn verify_encrypted_with_duplicate_clusters() -> Result<()> {
        let dir = tempdir()?;
        let img_path = dir.path().join("image.gb");
        let dest_path = dir.path().join("disk.raw");

        let block_size: u32 = 4096;
        let block_a = vec![0xCCu8; block_size as usize];
        let block_b = vec![0xDDu8; block_size as usize];
        let blocks = vec![
            block_a.clone(),
            block_a.clone(),
            block_b.clone(),
            block_a.clone(),
        ];

        build_synthetic_image(&img_path, &blocks, block_size, "secret")?;

        let mut handle = ImageHandle::open(&img_path)?;
        handle.load(Some("secret".to_string()))?;
        handle.write(&dest_path, false, |_, _| {})?;

        let all_verified = std::cell::Cell::new(true);
        handle.verify(&dest_path, |_, ok| {
            if let Some(false) = ok {
                all_verified.set(false);
            }
        })?;
        assert!(
            all_verified.get(),
            "all blocks must verify after a fresh write"
        );
        Ok(())
    }
}
