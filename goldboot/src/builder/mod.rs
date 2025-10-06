use self::qemu::{Accel, detect_accel};
use self::{fabricators::Fabricator, os::Os, sources::ImageSource};
use crate::builder::os::BuildImage;
use crate::library::ImageLibrary;

use anyhow::Result;
use byte_unit::Byte;
use goldboot_image::{ImageArch, ImageHandle, qcow::Qcow3};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    thread,
    time::SystemTime,
};
use tracing::info;
use validator::Validate;

pub mod fabricators;
pub mod http;
pub mod options;
pub mod os;
pub mod ovmf;
pub mod qemu;
pub mod sources;
pub mod ssh;
pub mod vnc;

///
#[derive(Clone, Serialize, Deserialize, Validate, Default, Debug)]
pub struct ImageElement {
    pub fabricators: Option<Vec<Fabricator>>,
    pub os: Os,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pref_size: Option<String>,
    pub source: ImageSource,
}

impl ImageElement {
    /// Get the size
    pub fn size(&self, image_size: String) -> u64 {
        let image_size = Byte::parse_str(image_size, true).unwrap();

        if let Some(_size) = &self.pref_size {
            todo!()
        } else {
            image_size.as_u64()
        }
    }
}

/// A `Foundry` produces a goldboot image given a raw configuration. This is the
/// central concept in the machinery that creates images.
#[derive(Clone, Serialize, Deserialize, Validate, Default, Debug)]
#[validate(schema(function = "crate::builder::custom_builder_validator"))]
pub struct Foundry {
    #[validate(length(min = 1))]
    pub alloy: Vec<ImageElement>,

    /// The system architecture
    #[serde(flatten)]
    pub arch: ImageArch,

    /// When set, the run will pause before each step in the boot sequence
    #[serde(default, skip_serializing)]
    pub debug: bool,

    /// An image description
    #[validate(length(max = 4096))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// The amount of memory to allocate to the VM
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<String>,

    /// The image name
    #[validate(length(min = 1, max = 64))]
    pub name: String,

    /// Don't use hardware acceleration even if available (slow)
    #[serde(default, skip_serializing)]
    pub no_accel: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub nvme: Option<bool>,

    /// The path to an OVMF.fd file
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ovmf_path: Option<String>,

    /// The encryption password
    #[serde(skip_serializing)]
    pub password: Option<String>,

    /// Whether the image is public
    #[serde(default, skip_serializing)]
    pub public: bool,

    /// Whether screenshots will be generated during the run for debugging
    #[serde(default, skip_serializing)]
    pub record: bool,

    /// The total image size in human-readable units
    pub size: String,
}

/// Handles more sophisticated validation of a [`Foundry`].
pub fn custom_builder_validator(_f: &Foundry) -> Result<(), validator::ValidationError> {
    // If there's more than one OS, they must all support alloy
    // if f.alloy.len() > 1 {
    //     for template in &self.config.templates {
    //         //if !template.is_multiboot() {
    //         //	bail!("Template does not support multiboot");
    //         //}
    //     }
    // }

    Ok(())
}

impl Foundry {
    fn new_worker(&self, element: ImageElement) -> FoundryWorker {
        // Obtain a temporary directory for the worker
        let tmp = tempfile::tempdir().unwrap();

        // Unpack included firmware if one isn't given
        let ovmf_path = if let Some(path) = self.ovmf_path.clone() {
            PathBuf::from(path)
        } else if let Some(path) = crate::builder::ovmf::find() {
            path
        } else if cfg!(feature = "include_ovmf") {
            let path = tmp.path().join("OVMF.fd").to_string_lossy().to_string();

            #[cfg(feature = "include_ovmf")]
            crate::builder::ovmf::write(self.arch, &path).unwrap();
            PathBuf::from(path)
        } else {
            panic!("No OVMF firmware found");
        };

        // Truncate the image size to a power of two for the qcow storage
        let qcow_size = element.size(self.size.clone());
        let qcow_size = qcow_size - (qcow_size % 2);

        FoundryWorker {
            arch: self.arch,
            accel: if self.no_accel {
                Accel::Tcg
            } else {
                detect_accel()
            },
            debug: self.debug,
            record: self.record,
            end_time: None,
            memory: self.memory.clone().unwrap_or(String::from("8G")),
            ovmf_path,
            qcow_path: tmp.path().join("image.gb.qcow2"),
            qcow_size,
            start_time: None,
            tmp,
            vnc_port: if self.debug {
                5900
            } else {
                rand::rng().random_range(5900..5999)
            },
            element,
        }
    }

    /// Run the entire build process. If no output file is given, the image is
    /// moved into the image library.
    pub fn run(&mut self, output: Option<String>) -> Result<()> {
        // Track the workers
        let mut workers = Vec::new();

        // TODO always sequential

        // If we're in debug mode, run workers sequentially
        if self.debug {
            for element in self.alloy.clone().into_iter() {
                let mut worker = self.new_worker(element);
                worker.run()?;
                workers.push(worker);
            }
        }
        // Otherwise run independent builds in parallel
        else {
            let mut handles = Vec::new();

            for element in self.alloy.clone().into_iter() {
                let mut worker = self.new_worker(element);
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
            Qcow3::open(&workers[0].qcow_path)?
        } else {
            Qcow3::open(&workers[0].qcow_path)?
        };

        // Convert into final immutable image
        let path = if let Some(output) = output.as_ref() {
            PathBuf::from(output)
        } else {
            ImageLibrary::open().temporary()
        };

        ImageHandle::from_qcow(
            Vec::new(),
            &final_qcow,
            &path,
            self.password.clone(),
            |_, _| {},
        )?;

        if let None = output {
            ImageLibrary::open().add_move(path.clone())?;
        }

        Ok(())
    }
}

// TODO remove worker altogether?

/// Manages the image build process. Multiple workers can run in parallel
/// to speed up multiboot configurations.
pub struct FoundryWorker {
    pub arch: ImageArch,

    pub accel: Accel,

    pub debug: bool,

    pub record: bool,

    pub element: ImageElement,

    /// The end time of the run
    pub end_time: Option<SystemTime>,

    pub memory: String,

    /// The path to the intermediate image artifact
    pub qcow_path: PathBuf,

    /// The size of the intermediate image in bytes
    pub qcow_size: u64,

    /// The start time of the run
    pub start_time: Option<SystemTime>,

    /// A general purpose temporary directory for the run
    pub tmp: tempfile::TempDir,

    /// The VM port for VNC
    pub vnc_port: u16,

    pub ovmf_path: PathBuf,
}

impl FoundryWorker {
    /// Run the image building process.
    pub fn run(&mut self) -> Result<()> {
        self.start_time = Some(SystemTime::now());
        Qcow3::create(&self.qcow_path, self.qcow_size)?;

        self.element.os.build(&self)?;
        info!(
            duration = ?self.start_time.unwrap().elapsed()?,
            "Build completed",
        );
        self.end_time = Some(SystemTime::now());

        Ok(())
    }
}
