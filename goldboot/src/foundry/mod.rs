use goldboot_image::{qcow::Qcow3, ImageArch, ImageHandle};
use log::{debug, info};
use molds::Mold;
use rand::Rng;
use serde::{Deserialize, Serialize};
use simple_error::bail;
use std::{error::Error, fmt::Display, thread, time::SystemTime};
use validator::Validate;

pub mod fabricators;
pub mod mold;
pub mod ovmf;
pub mod qemu;
pub mod sources;
pub mod ssh;
pub mod vnc;

/// A `Foundry` produces a goldboot image given a raw configuration. This is the
/// central concept in the machinery that creates images.
#[derive(Clone, Serialize, Deserialize, Validate, Default, Debug)]
pub struct Foundry {
    /// The image name
    #[validate(length(min = 1, max = 64))]
    pub name: String,

    /// An image description
    #[validate(length(max = 4096))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// The system architecture
    #[serde(flatten)]
    pub arch: ImageArch,

    /// The amount of memory to allocate to the VM
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub nvme: Option<bool>,

    /// The encryption password. This value can alternatively be specified on
    /// the command line and will be cleared before the config is included in
    /// an image file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,

    #[validate(length(min = 1))]
    pub alloy: Vec<Element>,

    pub source: Option<Source>,

    pub mold: Option<Mold>,

    #[validate(length(min = 1))]
    pub fabricators: Option<Vec<Fabricator>>,

    /// The path to an OVMF.fd file
    pub ovmf_path: String,

    /// Whether screenshots will be generated during the run for debugging
    pub record: bool,

    /// When set, the run will pause before each step in the boot sequence
    pub debug: bool,
}

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

    /// The path to the final image artifact
    pub image_path: String,
}

impl BuildJob {
    pub fn new(config: BuildConfig, record: bool, debug: bool) -> Self {
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
            image_path,
        }
    }

    /// Create a new generic build context.
    fn new_worker(&self, template: Box<dyn BuildTemplate>) -> Result<BuildWorker, Box<dyn Error>> {
        // Obtain a temporary directory
        let tmp = tempfile::tempdir().unwrap();

        // Determine image path
        let image_path = tmp.path().join("image.qcow2").to_string_lossy().to_string();

        // Unpack included firmware
        let ovmf_path = tmp.path().join("OVMF.fd").to_string_lossy().to_string();

        crate::ovmf::write_to(&self.config.arch, &ovmf_path)?;

        Ok(BuildWorker {
            tmp,
            image_path,
            ovmf_path,
            template,
            ssh_port: rand::thread_rng().gen_range(10000..11000),
            vnc_port: if self.debug {
                5900
            } else {
                rand::thread_rng().gen_range(5900..5999)
            },
            config: self.config.clone(),
            record: self.record,
            debug: self.debug,
        })
    }

    /// Run the entire build process. If no output file is given, the image is
    /// moved into the image library.
    pub fn run(&mut self, output: Option<String>) -> Result<(), Box<dyn Error>> {
        self.start_time = Some(SystemTime::now());

        // If there's more than one template, they must all support multiboot
        if self.config.templates.len() > 1 {
            for template in &self.config.templates {
                //if !template.is_multiboot() {
                //	bail!("Template does not support multiboot");
                //}
            }
        }

        // Track the workers
        let mut workers = Vec::new();

        // If we're in debug mode, run workers sequentially
        if self.debug {
            for template in self.config.templates.into_iter() {
                let worker = self.new_worker(template)?;
                worker.run()?;
                workers.push(worker);
            }
        }
        // Otherwise run independent builds in parallel
        else {
            let mut handles = Vec::new();

            for template in self.config.templates.into_iter() {
                let worker = self.new_worker(template)?;
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
        ImageHandle::convert(&final_qcow, self.config.clone(), &self.image_path)?;

        if let Some(output) = output {
            // Move the image to output
            std::fs::copy(&self.image_path, &output)?;
        } else {
            // Move the image to the library
            ImageLibrary::add(&self.image_path)?;
        }

        info!(
            "Build completed in: {:?}",
            self.start_time.unwrap().elapsed()?
        );
        self.end_time = Some(SystemTime::now());

        Ok(())
    }
}

/// Manages the image casting process. Multiple workers can run in parallel
/// to speed up multiboot configurations.
pub struct FoundryWorker {
    /// A general purpose temporary directory for the run
    pub tmp: tempfile::TempDir,

    /// The path to the intermediate image artifact
    pub image_path: String,

    /// The VM port for SSH
    pub ssh_port: u16,

    /// The VM port for VNC
    pub vnc_port: u16,

    pub element: Element,
}

impl FoundryWorker {
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

        qemuargs.drive.push(format!(
            "file={},if=virtio,cache=writeback,discard=ignore,format=qcow2",
            context.image_path
        ));
        qemuargs.drive.push(format!(
            "file={},media=cdrom",
            SourceCache::default()?.get(self.iso.url.clone(), &self.iso.checksum)?
        ));

        self.template.build(&self)?;
        Ok(())
    }
}