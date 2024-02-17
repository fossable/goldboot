use self::{fabricators::Fabricator, molds::ImageMold, sources::ImageSource};
use crate::cli::progress::ProgressBar;
use crate::foundry::molds::CastImage;

use anyhow::Result;
use byte_unit::Byte;
use clap::{builder::PossibleValue, ValueEnum};
use goldboot_image::{qcow::Qcow3, ImageArch, ImageHandle};
use rand::Rng;
use ron::ser::PrettyConfig;
use serde::{Deserialize, Serialize};
use std::{
    fmt::Display,
    path::{Path, PathBuf},
    sync::OnceLock,
    thread,
    time::SystemTime,
};
use strum::EnumIter;
use tracing::info;
use validator::Validate;

pub mod fabricators;
pub mod http;
pub mod molds;
pub mod options;
pub mod ovmf;
pub mod qemu;
pub mod sources;
pub mod ssh;
pub mod vnc;

///
#[derive(Clone, Serialize, Deserialize, Validate, Default, Debug)]
pub struct ImageElement {
    pub fabricators: Option<Vec<Fabricator>>,
    pub mold: ImageMold,
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
#[validate(schema(function = "crate::foundry::custom_foundry_validator"))]
pub struct Foundry {
    #[validate(length(min = 1))]
    pub alloy: Vec<ImageElement>,

    /// The system architecture
    #[serde(flatten)]
    pub arch: ImageArch,

    /// When set, the run will pause before each step in the boot sequence
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

    #[serde(skip_serializing_if = "Option::is_none")]
    pub nvme: Option<bool>,

    /// The path to an OVMF.fd file
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ovmf_path: Option<String>,

    /// The encryption password. This value can alternatively be specified on
    /// the command line and will be cleared before the config is included in
    /// an image file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,

    /// Whether screenshots will be generated during the run for debugging
    pub record: bool,

    pub size: String,
}

/// Handles more sophisticated validation of a [`Foundry`].
pub fn custom_foundry_validator(_f: &Foundry) -> Result<(), validator::ValidationError> {
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
    fn new_worker(&self, element: ImageElement) -> FoundryWorker {
        // Obtain a temporary directory for the worker
        let tmp = tempfile::tempdir().unwrap();

        // Determine image path
        let image_path = tmp.path().join("image.gb").to_string_lossy().to_string();

        // Unpack included firmware if one isn't given
        let ovmf_path = if let Some(path) = self.ovmf_path.clone() {
            path
        } else {
            let path = tmp.path().join("OVMF.fd").to_string_lossy().to_string();

            crate::foundry::ovmf::write_to(self.arch, &path).unwrap();
            path
        };

        FoundryWorker {
            arch: self.arch,
            debug: self.debug,
            memory: self.memory.clone().unwrap_or(String::from("4G")),
            tmp,
            start_time: None,
            end_time: None,
            vnc_port: if self.debug {
                5900
            } else {
                rand::thread_rng().gen_range(5900..5999)
            },
            qcow_path: image_path,
            qcow_size: element.size(self.size.clone()),
            element,
            ovmf_path,
        }
    }

    /// Run the entire build process. If no output file is given, the image is
    /// moved into the image library.
    pub fn run(&mut self, output: Option<String>) -> Result<()> {
        // Track the workers
        let mut workers = Vec::new();

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
        ImageHandle::convert(
            &final_qcow,
            self.name.clone(),
            ron::ser::to_string_pretty(&self, PrettyConfig::new())?.into_bytes(),
            self.password.clone(),
            output.unwrap(), // TODO ImageLibrary
            ProgressBar::Convert.new_empty(),
        )?;

        Ok(())
    }
}

/// Manages the image casting process. Multiple workers can run in parallel
/// to speed up multiboot configurations.
pub struct FoundryWorker {
    pub arch: ImageArch,

    pub debug: bool,

    pub element: ImageElement,

    /// The end time of the run
    pub end_time: Option<SystemTime>,

    pub memory: String,

    /// The path to the intermediate image artifact
    pub qcow_path: String,

    /// The size of the intermediate image in bytes
    pub qcow_size: u64,

    /// The start time of the run
    pub start_time: Option<SystemTime>,

    /// A general purpose temporary directory for the run
    pub tmp: tempfile::TempDir,

    /// The VM port for VNC
    pub vnc_port: u16,

    pub ovmf_path: String,
}

impl FoundryWorker {
    /// Run the image casting/building process.
    pub fn run(&mut self) -> Result<()> {
        self.start_time = Some(SystemTime::now());
        Qcow3::create(&self.qcow_path, self.qcow_size)?;

        self.element.mold.cast(&self)?;
        info!(
            "Build completed in: {:?}",
            self.start_time.unwrap().elapsed()?
        );
        self.end_time = Some(SystemTime::now());

        Ok(())
    }
}

/// Represents a foundry configuration file. This mainly helps sort out the various
/// supported config formats.
#[derive(Clone, Debug, EnumIter)]
pub enum FoundryConfig {
    Json(PathBuf),
    Ron(PathBuf),
    Toml(PathBuf),
    Yaml(PathBuf),
}

impl Default for FoundryConfig {
    fn default() -> Self {
        FoundryConfig::Ron(PathBuf::from("./goldboot.ron"))
    }
}

static VARIANTS: OnceLock<Vec<FoundryConfig>> = OnceLock::new();

impl ValueEnum for FoundryConfig {
    fn value_variants<'a>() -> &'a [Self] {
        VARIANTS.get_or_init(|| {
            vec![
                FoundryConfig::Json(PathBuf::from("./goldboot.json")),
                FoundryConfig::Ron(PathBuf::from("./goldboot.ron")),
                FoundryConfig::Toml(PathBuf::from("./goldboot.toml")),
                FoundryConfig::Yaml(PathBuf::from("./goldboot.yaml")),
            ]
        })
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        match *self {
            FoundryConfig::Json(_) => Some(PossibleValue::new("json")),
            FoundryConfig::Ron(_) => Some(PossibleValue::new("ron")),
            FoundryConfig::Toml(_) => Some(PossibleValue::new("toml")),
            FoundryConfig::Yaml(_) => Some(PossibleValue::new("yaml")),
        }
    }
}

impl FoundryConfig {
    /// Check for a foundry configuration file in the given directory.
    pub fn from_dir(path: impl AsRef<Path>) -> Option<FoundryConfig> {
        let path = path.as_ref();

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
        Ok(match &self {
            Self::Json(path) => serde_json::from_slice(&std::fs::read(path)?)?,
            Self::Ron(path) => ron::de::from_bytes(&std::fs::read(path)?)?,
            Self::Toml(path) => toml::from_str(String::from_utf8(std::fs::read(path)?)?.as_str())?,
            Self::Yaml(path) => serde_yaml::from_slice(&std::fs::read(path)?)?,
        })
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
        }?;
        Ok(())
    }
}

impl Display for FoundryConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let path = match self {
            FoundryConfig::Json(path) => path,
            FoundryConfig::Ron(path) => path,
            FoundryConfig::Toml(path) => path,
            FoundryConfig::Yaml(path) => path,
        }
        .to_string_lossy();
        path.fmt(f)
    }
}
