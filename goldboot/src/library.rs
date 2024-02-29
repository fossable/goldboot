use crate::cli::progress::ProgressBar;
use anyhow::anyhow;
use anyhow::bail;
use anyhow::Result;
use goldboot_image::ImageHandle;
use rand::Rng;
use sha1::Digest;
use sha2::Sha256;
use std::{
    fs::File,
    path::{Path, PathBuf},
};
use tracing::{debug, info};

/// Represents the local image library.
///
/// Depending on the platform, the directory will be located at:
///     - /var/lib/goldboot/images (linux)
///
/// Images are named according to their SHA256 hash (ID) and have a file
/// extension of ".gb".
pub struct ImageLibrary {
    pub directory: PathBuf,
}

impl ImageLibrary {
    pub fn open() -> Self {
        let directory = if cfg!(target_os = "linux") {
            PathBuf::from("/var/lib/goldboot/images")
        } else if cfg!(target_os = "macos") {
            PathBuf::from("/var/lib/goldboot/images")
        } else {
            panic!("Unsupported platform");
        };

        std::fs::create_dir_all(&directory).expect("failed to create image library");
        ImageLibrary { directory }
    }

    pub fn temporary(&self) -> PathBuf {
        let name: String = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(12)
            .map(char::from)
            .collect();
        self.directory.join(name)
    }

    /// Add an image to the library. The image will be hashed and copied to the
    /// library with the appropriate name.
    pub fn add_copy(&self, image_path: impl AsRef<Path>) -> Result<()> {
        info!("Saving image to library");

        let mut hasher = Sha256::new();
        ProgressBar::Hash.copy(
            &mut File::open(&image_path)?,
            &mut hasher,
            std::fs::metadata(&image_path)?.len(),
        )?;
        let hash = hex::encode(hasher.finalize());

        std::fs::copy(&image_path, self.directory.join(format!("{hash}.gb")))?;
        Ok(())
    }

    /// Add an image to the library. The image will be hashed and moved to the
    /// library with the appropriate name.
    pub fn add_move(&self, image_path: impl AsRef<Path>) -> Result<()> {
        info!("Saving image to library");

        let mut hasher = Sha256::new();
        ProgressBar::Hash.copy(
            &mut File::open(&image_path)?,
            &mut hasher,
            std::fs::metadata(&image_path)?.len(),
        )?;
        let hash = hex::encode(hasher.finalize());

        std::fs::rename(&image_path, self.directory.join(format!("{hash}.gb")))?;
        Ok(())
    }

    /// Remove an image from the library by ID.
    pub fn delete(&self, image_id: &str) -> Result<()> {
        for p in self.directory.read_dir()? {
            let path = p?.path();
            let filename = path.file_name().unwrap().to_str().unwrap();

            if filename == format!("{image_id}.gb")
                || filename == format!("{}.gb", &image_id[0..12])
            {
                std::fs::remove_file(path)?;
                return Ok(());
            }
        }

        Ok(())
    }

    /// Download a goldboot image over HTTP.
    pub fn download(&self, url: String) -> Result<ImageHandle> {
        let path = self.directory.join("goldboot-linux.gb");

        let mut rs = reqwest::blocking::get(&url)?;
        if rs.status().is_success() {
            let length = rs
                .content_length()
                .ok_or_else(|| anyhow!("Failed to get content length"))?;

            let mut file = File::create(&path)?;

            info!("Saving goldboot image");
            ProgressBar::Download.copy(&mut rs, &mut file, length)?;
            ImageHandle::open(&path)
        } else {
            bail!("Failed to download");
        }
    }

    /// Find images in the library by ID.
    pub fn find_by_id(image_id: &str) -> Result<ImageHandle> {
        Ok(Self::load()?
            .into_iter()
            .find(|image| image.id == image_id || image.id[0..12] == image_id[0..12])
            .ok_or_else(|| anyhow!("Image not found"))?)
    }

    /// Find images in the library by name.
    pub fn find_by_name(image_name: &str) -> Result<Vec<ImageHandle>> {
        Ok(Self::load()?
            .into_iter()
            .filter(|image| image.primary_header.name() == image_name)
            .collect())
    }

    /// Load images present in the local image library.
    pub fn load() -> Result<Vec<ImageHandle>> {
        let mut images = Vec::new();

        for p in Self::open().directory.read_dir()? {
            let path = p?.path();

            if let Some(ext) = path.extension() {
                if ext == "gb" {
                    match ImageHandle::open(&path) {
                        Ok(image) => images.push(image),
                        Err(error) => debug!("Failed to load image: {:?}", error),
                    }
                }
            }
        }

        Ok(images)
    }
}
