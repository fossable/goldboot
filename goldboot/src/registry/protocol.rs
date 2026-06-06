use goldboot_image::ImageArch;
use serde::{Deserialize, Serialize};

pub const API_VERSION: &str = "v1";
pub const MANIFEST_CONTENT_TYPE: &str = "application/vnd.goldboot.manifest";

// ── Images ──────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RegistryImageEntry {
    pub name: String,
    pub tag: String,
    /// Total decompressed size of the image, in bytes.
    pub size: u64,
    pub arch: ImageArch,
    /// Image creation time (Unix seconds).
    pub timestamp: u64,
    /// SHA256 of the underlying `.gb` file, hex-encoded.
    pub id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct ImageListResponse {
    pub images: Vec<RegistryImageEntry>,
}

// ── Errors ──────────────────────────────────────────────────────────────────

/// JSON body returned on 4xx/5xx responses.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ErrorResponse {
    pub message: String,
}
