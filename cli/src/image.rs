use anyhow::Result;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use validator::Validate;

/// Represents a local image.
#[derive(Clone, Serialize, Deserialize, Validate)]
pub struct ImageMetadata {
    pub name: String,

    pub sha256: String,

    pub generate_time: u64,

    pub parent_image: String,
}

impl ImageMetadata {
    /// Load images present in the local image library
    pub fn load() -> Result<Vec<ImageMetadata>> {
        let image_path = Path::new("/var/lib/goldboot/images");

        let mut images = Vec::new();

        for p in image_path.read_dir().unwrap() {
            let path = p.unwrap().path();

            if let Some(ext) = path.extension() {
                let filename = path.file_stem().unwrap().to_str().unwrap().to_string();
                if ext == "json" {
                    // Hash the file and compare it to the filename
                    let content = fs::read(&path).unwrap();

                    if *Sha256::new().chain_update(content).finalize() == *filename.as_bytes() {
                        let metadata: ImageMetadata =
                            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
                        images.push(metadata);
                    }
                }
            }
        }

        Ok(images)
    }
}
