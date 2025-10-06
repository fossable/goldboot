use goldboot_image::{ImageArch, ImageHandle};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct ImageInfoResponse {
    pub version: u8,

    /// The total size of all blocks combined in bytes
    pub size: u64,

    /// Image creation time
    pub timestamp: u64,

    /// A copy of the name field from the config
    pub name: String,

    /// System architecture
    pub arch: ImageArch,
}

impl From<ImageHandle> for ImageInfoResponse {
    fn from(value: ImageHandle) -> Self {
        Self {
            version: value.primary_header.version,
            size: value.primary_header.size,
            timestamp: value.primary_header.timestamp,
            name: todo!(),
            arch: value.primary_header.arch,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct ImageListResponse {
    pub results: Vec<ImageInfoResponse>,
}
