use crate::registry::extract;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use goldboot_image::{ImageArch, ImageHandle};
use serde::{Deserialize, Serialize};

use crate::library::ImageLibrary;

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
            name: value.primary_header.name(),
            arch: value.primary_header.arch,
        }
    }
}

/// Get image info
pub async fn info(image: extract::ImageHandle) -> Json<ImageInfoResponse> {
    Json(image.0.into())
}

/// Get image list
pub async fn list() {}

// Push an image
/*
pub async fn push(id: web::Path<String>, rq: actix_web::HttpRequest) -> Result<HttpResponse> {
    let path = match ImageLibrary::find_by_id(&id) {
        Ok(image) => {
            // Delete if the image already exists
            if Path::new(&image.path).exists() {
                std::fs::remove_file(&image.path)?;
            }
            image.path
        },
        _ => format!("{}.gb", id),
    };

    let mut file = File::create(&path)?;
    std::io::copy(&mut rq, &mut file)?;
    ""
}*/

/// Get cluster data
pub async fn clusters(Path(_id): Path<String>, Path(_range): Path<String>) {}

/// Get cluster hashes
pub async fn hashes(Path(_id): Path<String>, Path(_range): Path<String>) {}
