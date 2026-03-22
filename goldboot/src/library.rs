use crate::cli::progress::ProgressBar;
use anyhow::{Result, anyhow, bail};
use goldboot_image::ImageHandle;
use rand::Rng;
use sha1::Digest;
use sha2::Sha256;
use std::{
    fs::File,
    path::{Path, PathBuf},
};
use tracing::{debug, info};

/// Return the path to the goldboot build cache directory, creating it if needed.
pub fn cache_dir() -> PathBuf {
    let dir =
        PathBuf::from(std::env::var("HOME").expect("HOME not set")).join(".cache/goldboot/images");
    std::fs::create_dir_all(&dir).expect("failed to create cache directory");
    dir
}

/// Return the persistent qcow2 path for a given context directory.
///
/// The path is stable across runs: `~/.goldboot/cache/<sha256(canonical_path)>.qcow2`.
pub fn qcow_cache_path(context_dir: &Path) -> Result<PathBuf> {
    let canonical = context_dir.canonicalize()?;
    let hash = hex::encode(
        Sha256::new()
            .chain_update(canonical.to_string_lossy().as_bytes())
            .finalize(),
    );
    Ok(cache_dir().join(format!("{hash}.qcow2")))
}

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
        let name: String = rand::rng()
            .sample_iter(&rand::distr::Alphanumeric)
            .take(12)
            .map(char::from)
            .collect();
        self.directory.join(name)
    }

    /// Add an image to the library. The image will be hashed and copied to the
    /// library with the appropriate name.
    pub fn add_copy(&self, image_path: impl AsRef<Path>) -> Result<()> {
        let mut hasher = Sha256::new();
        ProgressBar::Hash.copy(
            &mut File::open(&image_path)?,
            &mut hasher,
            std::fs::metadata(&image_path)?.len(),
        )?;
        let hash = hex::encode(hasher.finalize());
        let dest = self.directory.join(format!("{hash}.gb"));

        info!(path = %dest.display(), "Copying image to library");
        std::fs::copy(&image_path, &dest)?;
        Ok(())
    }

    /// Add an image to the library. The image will be hashed and moved to the
    /// library with the appropriate name.
    pub fn add_move(&self, image_path: impl AsRef<Path>) -> Result<()> {
        let mut hasher = Sha256::new();
        ProgressBar::Hash.copy(
            &mut File::open(&image_path)?,
            &mut hasher,
            std::fs::metadata(&image_path)?.len(),
        )?;
        let hash = hex::encode(hasher.finalize());
        let dest = self.directory.join(format!("{hash}.gb"));

        info!(path = %dest.display(), "Moving image to library");
        std::fs::rename(&image_path, &dest)?;
        Ok(())
    }

    /// Remove an image from the library by ID.
    pub fn delete(&self, image_id: &str) -> Result<()> {
        let image = Self::find_by_id(image_id)?;
        std::fs::remove_file(&image.path)?;
        debug!(path = %image.path.display(), "Deleted image");
        Ok(())
    }

    /// Download a goldboot image over HTTP.
    pub fn download(&self, url: String) -> Result<ImageHandle> {
        let path = self.directory.join("goldboot-uki.gb");

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

    /// Find an image in the library by name (matches `PrimaryHeader::name()`).
    /// Returns an error if zero or more than one image matches.
    pub fn find_by_name(name: &str) -> Result<ImageHandle> {
        let mut matches: Vec<ImageHandle> = Self::find_all()?
            .into_iter()
            .filter(|image| image.primary_header.name() == name)
            .collect();
        match matches.len() {
            0 => bail!("No image named '{}' found in library", name),
            1 => Ok(matches.remove(0)),
            n => bail!(
                "{} images named '{}' found; use 'goldboot image list' and push by ID instead",
                n,
                name
            ),
        }
    }

    /// Find images in the library by ID.
    pub fn find_by_id(image_id: &str) -> Result<ImageHandle> {
        Ok(Self::find_all()?
            .into_iter()
            .find(|image| image.id == image_id || image.id[0..12] == image_id[0..12])
            .ok_or_else(|| anyhow!("Image not found"))?)
    }

    /// Find images in the library that have the given OS.
    pub fn find_by_os(os: &str) -> Result<Vec<ImageHandle>> {
        Ok(Self::find_all()?
            .into_iter()
            .filter(|image| {
                image
                    .primary_header
                    .elements
                    .iter()
                    .any(|element| element.os() == os)
            })
            .collect())
    }

    /// Find all images present in the local image library.
    pub fn find_all() -> Result<Vec<ImageHandle>> {
        let mut images = Vec::new();

        for p in Self::open().directory.read_dir()? {
            let path = p?.path();

            if let Some(ext) = path.extension() {
                if ext == "gb" {
                    images.push(ImageHandle::open(&path)?);
                }
            }
        }

        Ok(images)
    }
}
