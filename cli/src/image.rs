use std::path::PathBuf;
use crate::image_library_path;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use validator::Validate;
use log::debug;

/// Represents a local image.
#[derive(Clone, Serialize, Deserialize, Validate)]
pub struct ImageMetadata {
    pub name: String,

    pub sha256: String,

    pub generate_time: u64,

    pub parent_image: String,
}

impl ImageMetadata {

    /// Write the image metadata to the library and return the metadata hash
    pub fn write(&self) -> Result<String> {
        let metadata_json = serde_json::to_string(&self).unwrap();
        let hash = hex::encode(Sha256::new().chain_update(&metadata_json).finalize());

        // Write it to the library directory
        fs::write(image_library_path().join(format!("{}.json", hash)), &metadata_json).unwrap();
        Ok(hash)
    }

    pub fn new(output: PathBuf) -> Result<ImageMetadata> {
        Ok(ImageMetadata{
            name: "".into(),
            sha256: "".into(),
            generate_time: 0u64,
            parent_image: "".into(),
        })
    }

    /// Load images present in the local image library
    pub fn load() -> Result<Vec<ImageMetadata>> {

        let mut images = Vec::new();

        for p in image_library_path().read_dir().unwrap() {
            let path = p.unwrap().path();

            if let Some(ext) = path.extension() {
                let filename = path.file_stem().unwrap().to_str().unwrap().to_string();
                if ext == "json" {
                    // Hash the file and compare it to the filename
                    let content = fs::read(&path).unwrap();

                    if *Sha256::new().chain_update(content).finalize() == hex::decode(filename).unwrap() {
                        let metadata: ImageMetadata =
                            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
                        images.push(metadata);
                    } else {
                        debug!("Found corrupt file in image directory: {}", path.display());
                    }
                }
            }
        }

        Ok(images)
    }
}
