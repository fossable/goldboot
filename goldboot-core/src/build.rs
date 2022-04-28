use crate::{compact_image, find_ovmf, image::ImageLibrary, BuildConfig, Template};
use goldboot_image::GoldbootImage;
use log::{debug, info};
use rand::Rng;
use simple_error::bail;
use std::{error::Error, thread, time::SystemTime};

/// Represents an image build job.
pub struct BuildJob {
	/// A general purpose temporary directory for the run
	pub tmp: tempfile::TempDir,

	/// The start time of the run
	pub start_time: Option<SystemTime>,

	/// The end time of the run
	pub end_time: Option<SystemTime>,

	/// The build config
	pub config: BuildConfig,

	/// Whether screenshots will be generated during the run for debugging
	pub record: bool,

	/// When set, the run will pause before each step in the boot sequence
	pub debug: bool,

	/// When set, the run will output user-friendly progress bars
	pub interactive: bool,

	/// The path to the final image artifact
	pub image_path: String,
}

impl BuildJob {
	pub fn new(config: BuildConfig, record: bool, debug: bool, interactive: bool) -> Self {
		// Obtain a temporary directory
		let tmp = tempfile::tempdir().unwrap();

		// Determine image path
		let image_path = tmp.path().join("image.gb").to_string_lossy().to_string();

		Self {
			tmp,
			start_time: None,
			end_time: None,
			config,
			record,
			debug,
			interactive,
			image_path,
		}
	}

	/// Create a new generic build context.
	fn new_worker(&self, template: Box<dyn Template>) -> BuildWorker {
		// Obtain a temporary directory
		let tmp = tempfile::tempdir().unwrap();

		// Determine image path
		let image_path = tmp.path().join("image.gb").to_string_lossy().to_string();

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

		BuildWorker {
			tmp,
			image_path,
			ovmf_path,
			ssh_port: rand::thread_rng().gen_range(10000..11000),
			vnc_port: rand::thread_rng().gen_range(5900..5999),
			config: self.config.clone(),
			record: self.record,
			debug: self.debug,
			interactive: self.interactive,
		}
	}

	/// Run the entire build process.
	pub fn run(&mut self) -> Result<(), Box<dyn Error>> {
		self.start_time = Some(SystemTime::now());

		let tmp = tempfile::tempdir().unwrap();

		// Load templates
		let templates = self.config.get_templates()?;

		// If there's more than one template, they must all support multiboot
		if templates.len() > 1 {
			for template in templates {
				if template.multiboot_container().is_none() {
					bail!("Template does not support multiboot");
				}
			}
		}

		// Create a worker for each template
		let workers = templates.map(|t| self.new_worker(t)).collect();

		// If we're in debug mode, run workers sequentially
		if self.debug {
			for worker in workers {
				worker.run();
				worker.thread_handle.join().unwrap();
			}
		}
		// Otherwise run independent builds in parallel
		else {
			for worker in workers {
				worker.run();
			}

			// Wait for each build to complete
			for worker in workers {
				worker.thread_handle.join().unwrap();
			}
		}

		// Allocate a final image if we're multibooting
		if templates.len() > 1 {
			/*GoldbootImage::create(
				&self.image_path,
				self.config.disk_size_bytes(),
				serde_json::to_vec(&self.config)?,
			)?;*/
		} else {
		}

		// Attempt to reduce the size of image
		compact_image(&self.image_path)?;

		info!(
			"Build completed in: {:?}",
			self.start_time.unwrap().elapsed()
		);
		self.end_time = Some(SystemTime::now());

		// Move the image to the library
		ImageLibrary::add(&self.image_path)?;

		Ok(())
	}
}

/// Represents a template build process. Multiple workers can run in parallel
/// to speed up multiboot configurations.
pub struct BuildWorker {
	/// A general purpose temporary directory for the run
	pub tmp: tempfile::TempDir,

	/// The path to the intermediate image artifact
	pub image_path: String,

	/// The VM port for SSH
	pub ssh_port: u16,

	/// The VM port for VNC
	pub vnc_port: u16,

	/// The build config
	pub config: BuildConfig,

	/// The template to build
	pub template: Box<dyn Template>,

	/// The path to an OVMF.fd file
	pub ovmf_path: String,

	/// Whether screenshots will be generated during the run for debugging
	pub record: bool,

	/// When set, the run will pause before each step in the boot sequence
	pub debug: bool,

	/// When set, the run will output user-friendly progress bars
	pub interactive: bool,

	pub thread_handle: thread::JoinHandle<()>,
}

impl BuildWorker {
	/// Run the template build.
	pub fn run(&mut self) -> Result<(), Box<dyn Error>> {
		let image_size = if let Some(multiboot) = self.template.multiboot_container() {
			(multiboot.percentage() * self.config.disk_size_bytes() as f64) as u64
		} else {
			self.config.disk_size_bytes()
		};

		debug!("Allocating new {} image: {}", image_size, self.image_path);
		GoldbootImage::create(
			&self.image_path,
			image_size,
			serde_json::to_vec(&self.config)?,
		)?;

		self.thread_handle = thread::spawn(|| self.template.build(&self)?);
		Ok(())
	}
}
