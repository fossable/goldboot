use crate::{compact_image, find_ovmf, BuildConfig, ImageMetadata};
use log::{debug, info};
use rand::Rng;
use std::{error::Error, time::Instant};

pub struct BuildJob {
	/// The start time of the run
	pub start_time: Option<chrono::DateTime>,

	/// The end time of the run
	pub end_time: Option<chrono::DateTime>,
}

impl BuildJob {
	pub fn new(config: BuildConfig, record: bool, debug: bool) -> Self {
		// Determine firmware path or use included firmware
		let ovmf_path = if let Some(path) = find_ovmf() {
			path
		} else {
			if cfg!(target_arch = "x86_64") {
				debug!("Unpacking included firmware");
				let resource_path = tmp.path().join("OVMF.fd");
				let resource = Resources::get("OVMF.fd").unwrap();
				std::fs::write(&resource_path, resource.data).unwrap();
				resource_path.to_string_lossy().to_string()
			} else {
				panic!("Firmware not found");
			}
		};

		Self {
			start_time: None,
			end_time: None,
			config,
			ovmf_path,
			record,
			debug,
		}
	}

	pub fn run(&self) {}
}

pub struct BuildContext {
	/// A general purpose temporary directory for the run
	pub tmp: tempfile::TempDir,

	pub image_path: String,

	pub ssh_port: u16,

	pub vnc_port: u16,

	pub config: BuildConfig,

	pub ovmf_path: String,

	/// Whether screenshots will be generated during the run for debugging
	pub record: bool,

	/// When set, the run will pause before each step in the boot sequence
	pub debug: bool,
}

impl BuildContext {
	pub fn new(job: &BuildJob) -> Self {
		// Obtain a temporary directory
		let tmp = tempfile::tempdir().unwrap();

		// Determine image path
		let image_path = tmp.path().join("image.gb").to_string_lossy().to_string();

		Self {
			job,
			tmp,
			image_path,
			ssh_port: rand::thread_rng().gen_range(10000..11000),
			vnc_port: rand::thread_rng().gen_range(5900..5999),
		}
	}

	pub fn start(&self) -> Result<(), Box<dyn Error>> {
		let start_time = Instant::now();
		let context = BuildContext::new(config, record, debug);

		// Prepare to build templates
		let profiles = goldboot_templates::get_templates(&context.config)?;
		let profiles_len = profiles.len();
		if profiles_len == 0 {
			bail!("At least one base profile must be specified");
		}

		// Create an initial image that will be attached as storage to each VM
		debug!(
			"Allocating new {} image: {}",
			context.config.disk_size, context.image_path
		);
		goldboot_image::GoldbootImage::create(
			&context.image_path,
			context.config.disk_size_bytes(),
			serde_json::to_vec(&context.config)?,
		)?;

		// Create partitions if we're multi booting
		if profiles.len() > 1 {
			// TODO
		}

		// Build each profile
		for profile in profiles {
			profile.build(&context)?;
		}

		// Install bootloader if we're multi booting
		if profiles_len > 1 {
			// TODO
		}

		// Attempt to reduce the size of image
		compact_image(&context.image_path)?;

		info!("Build completed in: {:?}", start_time.elapsed());

		// Create new image metadata
		// TODO
		let metadata = ImageMetadata {
			sha256: String::from(""),
			size: 0,
			last_modified: 0,
			config: context.config,
		};

		// Move the image to the library
		std::fs::copy(context.image_path, metadata.path_qcow2())?;

		Ok(())
	}
}
