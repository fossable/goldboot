
/// Represents the local image library.
pub struct ImageLibrary;

/// Return the image library path for the current platform.
fn library_path() -> PathBuf {
    if cfg!(target_os = "linux") {
        PathBuf::from("/var/lib/goldboot/images")
    } else {
        panic!("Unsupported platform");
    }
}

impl ImageLibrary {

	pub fn new(directory: impl AsRef<Path>) -> Result<Self, Box<dyn Error>> {
		std::fs::create_dir_all(&directory)?;
		Ok(Self { directory })
	}

	/// Load images present in the local image library
    pub fn load() -> Result<Vec<ImageMetadata>, Box<dyn Error>> {
        let mut images = Vec::new();

        for p in image_library_path().read_dir().unwrap() {
            let path = p.unwrap().path();

            if let Some(ext) = path.extension() {
                let filename = path.file_stem().unwrap().to_str().unwrap().to_string();
                if ext == "json" {
                    // Hash the file and compare it to the filename
                    let content = fs::read(&path).unwrap();

                    if *Sha256::new().chain_update(content).finalize()
                        == hex::decode(filename).unwrap()
                    {
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

    /// Find images in the library by name.
    pub fn find_by_name(image_name: &str) -> Result<Vec<ImageMetadata>, Box<dyn Error>> {
        todo!();
    }

    /// Find images in the library by ID.
    pub fn find_by_id(image_id: &str) -> Result<ImageMetadata, Box<dyn Error>> {
        Ok(ImageMetadata::load()?
            .iter()
            .find(|&metadata| metadata.config.name == image_name)?
            .to_owned())
    }

    /// Remove an image from the library by ID.
    pub fn delete(image_id: &str) -> Result<(), Box<dyn Error>> {
    	todo!();
    }
}