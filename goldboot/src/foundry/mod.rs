use anyhow::bail;
use anyhow::Result;
use goldboot_image::{qcow::Qcow3, ImageArch, ImageHandle};
use log::{debug, info};
use rand::Rng;
use ron::ser::PrettyConfig;
use serde::{Deserialize, Serialize};
use std::{
    error::Error,
    fmt::Display,
    path::{Path, PathBuf},
    thread,
    time::SystemTime,
};
use validator::Validate;

use crate::{foundry::sources::SourceCache, library::ImageLibrary};

use self::{fabricators::Fabricator, molds::ImageMold, sources::Source};

pub mod fabricators;
pub mod molds;
pub mod options;
pub mod ovmf;
pub mod qemu;
pub mod sources;
pub mod ssh;
pub mod vnc;

/// A `Foundry` produces a goldboot image given a raw configuration. This is the
/// central concept in the machinery that creates images.
#[derive(Clone, Serialize, Deserialize, Validate, Default, Debug)]
#[validate(schema(function = "crate::foundry::custom_foundry_validator"))]
pub struct Foundry {
    /// The system architecture
    #[serde(flatten)]
    pub arch: ImageArch,

    /// When set, the run will pause before each step in the boot sequence
    pub debug: bool,

    /// An image description
    #[validate(length(max = 4096))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    #[validate(length(min = 1))]
    pub fabricators: Option<Vec<Fabricator>>,

    /// The amount of memory to allocate to the VM
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<String>,

    pub mold: Option<ImageMold>,

    /// The image name
    #[validate(length(min = 1, max = 64))]
    pub name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub nvme: Option<bool>,

    /// The path to an OVMF.fd file
    pub ovmf_path: String,

    /// The encryption password. This value can alternatively be specified on
    /// the command line and will be cleared before the config is included in
    /// an image file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,

    /// Whether screenshots will be generated during the run for debugging
    pub record: bool,

    // #[validate(length(min = 2))]
    // pub alloy: Vec<Element>,
    pub source: Option<Source>,
}

/// Handles more sophisticated validation of a [`Foundry`].
pub fn custom_foundry_validator(f: &Foundry) -> Result<(), validator::ValidationError> {
    // If there's more than one mold, they must all support alloy
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
    fn new_worker(&self) -> FoundryWorker {
        // Obtain a temporary directory for the worker
        let tmp = tempfile::tempdir().unwrap();

        // Determine image path
        let image_path = tmp.path().join("image.gb").to_string_lossy().to_string();

        // Unpack included firmware
        let ovmf_path = tmp.path().join("OVMF.fd").to_string_lossy().to_string();

        crate::ovmf::write_to(&self.config.arch, &ovmf_path)?;

        FoundryWorker {
            tmp,
            start_time: None,
            end_time: None,
            ssh_port: rand::thread_rng().gen_range(10000..11000),
            vnc_port: if self.debug {
                5900
            } else {
                rand::thread_rng().gen_range(5900..5999)
            },
            image_path,
        }
    }

    /// Run the entire build process. If no output file is given, the image is
    /// moved into the image library.
    pub fn run(&mut self, output: Option<String>) -> Result<()> {
        self.start_time = Some(SystemTime::now());

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

    /// The start time of the run
    pub start_time: Option<SystemTime>,

    /// The end time of the run
    pub end_time: Option<SystemTime>,
}

impl FoundryWorker {
    /// Run the template build.
    pub fn run(&self) -> Result<()> {
        debug!(
            "Allocating new {} image: {}",
            self.template.general().storage_size,
            self.image_path
        );
        Qcow3::create(
            &self.image_path,
            self.template.general().storage_size_bytes(),
        )?;

        // qemuargs.drive.push(format!(
        //     "file={},if=virtio,cache=writeback,discard=ignore,format=qcow2",
        //     context.image_path
        // ));
        // qemuargs.drive.push(format!(
        //     "file={},media=cdrom",
        //     SourceCache::default()?.get(self.iso.url.clone(), &self.iso.checksum)?
        // ));

        self.template.build(&self)?;
        Ok(())
    }
}

/// Represents a foundry configuration file. This mainly helps sort out the various
/// supported config formats.
pub enum FoundryConfig {
    Json(PathBuf),
    Ron(PathBuf),
    Toml(PathBuf),
    Yaml(PathBuf),
}

impl FoundryConfig {
    /// Check for a foundry configuration file in the given directory.
    pub fn from_dir(path: impl AsRef<Path>) -> Option<FoundryConfig> {
        path = path.as_ref();

        if path.join("goldboot.json").exists() {
            Some(FoundryConfig::Json(path.join("goldboot.json")))
        } else if path.join("goldboot.ron").exists() {
            Some(FoundryConfig::Ron(path.join("goldboot.ron")))
        } else if path.join("goldboot.toml").exists() {
            Some(FoundryConfig::Toml(path.join("goldboot.toml")))
        } else if path.join("goldboot.yaml").exists() {
            Some(FoundryConfig::Yaml(path.join("goldboot.yaml")))
        } else if path.join("goldboot.yml").exists() {
            Some(FoundryConfig::Yaml(path.join("goldboot.yml")))
        } else {
            None
        }
    }

    /// Read the configuration file into a new [`Foundry`].
    pub fn load(&self) -> Result<Foundry> {
        match &self {
            Self::Json(path) => serde_json::from_slice(std::fs::read(path)),
            Self::Ron(path) => ron::de::from_bytes(std::fs::read(path)),
            Self::Toml(path) => toml::from_str(String::from_utf8(std::fs::read(path))?.as_str()),
            Self::Yaml(path) => serde_yaml::from_slice(std::fs::read(path)),
        }
    }

    /// Write a [`Foundry`] to a configuration file.
    pub fn write(&self, foundry: &Foundry) -> Result<()> {
        match &self {
            Self::Json(path) => std::fs::write(path, serde_json::to_vec_pretty(foundry)?),
            Self::Ron(path) => std::fs::write(
                path,
                ron::ser::to_string_pretty(foundry, PrettyConfig::new())?.into_bytes(),
            ),
            Self::Toml(path) => std::fs::write(path, toml::to_string_pretty(foundry)?.into_bytes()),
            Self::Yaml(path) => std::fs::write(path, serde_yaml::to_string(foundry)?.into_bytes()),
        }
    }
}
