use crate::{image::GoldbootImage, progress::ProgressBar};
use log::{debug, info};
use sha1::Digest;
use sha2::Sha256;
use simple_error::bail;
use std::{
	error::Error,
	fs::File,
	path::{Path, PathBuf},
};

/// Represents the local image library.
///
/// Depending on the platform, the directory will be located at:
///     - /var/lib/goldboot/images (linux)
///
/// Images are named according to their SHA256 hash (ID) and have a file extension
/// of ".gb".
pub struct ImageLibrary;

/// Return the image library path for the current platform.
fn library_path() -> PathBuf {
	let path = if cfg!(target_os = "linux") {
		PathBuf::from("/var/lib/goldboot/images")
	} else if cfg!(target_os = "macos") {
		PathBuf::from("/var/lib/goldboot/images")
	} else {
		panic!("Unsupported platform");
	};

	std::fs::create_dir_all(&path).unwrap();
	path
}

impl ImageLibrary {
	/// Add an image to the library. The image will be hashed and copied to the
	/// library with the appropriate name.
	pub fn add(image_path: impl AsRef<Path>) -> Result<(), Box<dyn Error>> {
		info!("Saving image to library");

		let mut hasher = Sha256::new();
		ProgressBar::Hash.copy(
			&mut File::open(&image_path)?,
			&mut hasher,
			std::fs::metadata(&image_path)?.len(),
		)?;
		let hash = hex::encode(hasher.finalize());

		std::fs::copy(&image_path, library_path().join(format!("{hash}.gb")))?;
		Ok(())
	}

	/// Download a goldboot image over HTTP.
	pub fn download(url: String) -> Result<GoldbootImage, Box<dyn Error>> {
		let path = library_path().join("goldboot-linux.gb");

		let mut rs = reqwest::blocking::get(&url)?;
		if rs.status().is_success() {
			let length = rs.content_length().ok_or("Failed to get content length")?;

			let mut file = File::create(&path)?;

			info!("Saving goldboot image");
			ProgressBar::Download.copy(&mut rs, &mut file, length)?;
			GoldbootImage::open(&path)
		} else {
			bail!("Failed to download");
		}
	}

	/// Load images present in the local image library.
	pub fn load() -> Result<Vec<GoldbootImage>, Box<dyn Error>> {
		let mut images = Vec::new();

		for p in library_path().read_dir()? {
			let path = p?.path();

			if let Some(ext) = path.extension() {
				if ext == "gb" {
					match GoldbootImage::open(&path) {
						Ok(image) => images.push(image),
						Err(error) => debug!("Failed to load image: {:?}", error),
					}
				}
			}
		}

		Ok(images)
	}

	/// Find images in the library by name.
	pub fn find_by_name(image_name: &str) -> Result<Vec<GoldbootImage>, Box<dyn Error>> {
		Ok(ImageLibrary::load()?
			.into_iter()
			.filter(|image| image.metadata.config.name == image_name)
			.collect())
	}

	/// Find images in the library by ID.
	pub fn find_by_id(image_id: &str) -> Result<GoldbootImage, Box<dyn Error>> {
		Ok(ImageLibrary::load()?
			.into_iter()
			.find(|image| image.id == image_id || image.id[0..12] == image_id[0..12])
			.ok_or("Image not found")?)
	}

	/// Remove an image from the library by ID.
	pub fn delete(image_id: &str) -> Result<(), Box<dyn Error>> {
		todo!();
	}
}
