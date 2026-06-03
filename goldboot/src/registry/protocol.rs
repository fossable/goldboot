use goldboot_image::ImageArch;
use serde::{Deserialize, Serialize};

pub const API_VERSION: &str = "v1";
pub const MANIFEST_CONTENT_TYPE: &str = "application/vnd.goldboot.manifest";

// ── Auth ────────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LoginResponse {
    pub token: String,
    /// Unix timestamp (seconds) at which the token expires.
    pub expires_at: u64,
    pub permissions: Permissions,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Permissions {
    #[serde(default)]
    pub pull: bool,
    #[serde(default)]
    pub push: bool,
}

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

/// JSON body returned on 4xx/5xx responses. `message` is intentionally
/// generic so that auth failures don't leak whether a username exists.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ErrorResponse {
    pub message: String,
}
