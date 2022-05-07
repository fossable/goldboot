use goldboot_image::GoldbootImage;
use crate::{find_ovmf, image::ImageLibrary, BuildConfig, Template};
use goldboot_image::qcow::Qcow3;
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
		let image_path = tmp.path().join("image.qcow2").to_string_lossy().to_string();

		// Determine firmware path or use included firmware
		let ovmf_path = if let Some(path) = find_ovmf() {
			path
		} else {
			if cfg!(target_arch = "x86_64") {
				debug!("Unpacking included firmware");
				/*let resource_path = tmp.path().join("OVMF.fd");
				let resource = Resources::get("OVMF.fd").unwrap();
				std::fs::write(&resource_path, resource.data).unwrap();
				resource_path.to_string_lossy().to_string()*/
				panic!();
			} else {
				panic!("Firmware not found");
			}
		};

		BuildWorker {
			tmp,
			image_path,
			ovmf_path,
			template,
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

		// Load templates
		let templates = self.config.get_templates()?;

		// If there's more than one template, they must all support multiboot
		if templates.len() > 1 {
			for template in &templates {
				if !template.is_multiboot() {
					bail!("Template does not support multiboot");
				}
			}
		}

		// Track the workers
		let mut workers = Vec::new();

		// If we're in debug mode, run workers sequentially
		if self.debug {
			for template in templates.into_iter() {
				let worker = self.new_worker(template);
				worker.run()?;
				workers.push(worker);
			}
		}
		// Otherwise run independent builds in parallel
		else {
			let mut handles = Vec::new();

			for template in templates.into_iter() {
				let worker = self.new_worker(template);
				handles.push(thread::spawn(move || {
					worker.run().unwrap();
					worker
				}));
			}

			// Wait for each build to complete
			for handle in handles {
				workers.push(handle.join().unwrap());
			}
		}

		let final_qcow = if workers.len() > 1 {
			// Allocate a temporary image if we need to merge
			// TODO
			Qcow3::open(&workers[0].image_path)?
		} else {
			Qcow3::open(&workers[0].image_path)?
		};

		// Convert into final immutable image
		GoldbootImage::convert(&final_qcow, &self.image_path)?;

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

	pub template: Box<dyn Template>,

	/// The path to an OVMF.fd file
	pub ovmf_path: String,

	/// Whether screenshots will be generated during the run for debugging
	pub record: bool,

	/// When set, the run will pause before each step in the boot sequence
	pub debug: bool,

	/// When set, the run will output user-friendly progress bars
	pub interactive: bool,
}

unsafe impl Send for BuildWorker {}

impl BuildWorker {
	/// Run the template build.
	pub fn run(&self) -> Result<(), Box<dyn Error>> {
		debug!(
			"Allocating new {} image: {}",
			self.template.general().storage_size,
			self.image_path
		);
		Qcow3::create(
			&self.image_path,
			self.template.general().storage_size_bytes(),
		)?;

		self.template.build(&self)?;
		Ok(())
	}
}
