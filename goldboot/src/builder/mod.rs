use self::qemu::{Accel, detect_accel};
use crate::builder::os::OsConfig;
use crate::cli::cmd::Commands;
use crate::library::{ImageLibrary, qcow_cache_path};

use anyhow::{Result, bail};
use dialoguer::Password;
use goldboot_image::{ElementHeader, ImageArch, ImageHandle, qcow::Qcow3};
use rand::Rng;
use std::{path::PathBuf, time::SystemTime};
use tracing::info;
use validator::Validate;

pub mod config;
pub mod fabricators;
pub mod http;
pub mod options;
pub mod os;
pub mod ovmf;
pub mod qemu;
pub mod sources;
pub mod ssh;
pub mod vnc;

/// Machinery that creates Goldboot images from image elements.
#[derive(Validate)]
pub struct Builder {
    pub elements: Vec<OsConfig>,

    pub accel: Accel,

    pub debug: bool,

    pub record: bool,

    /// Context directory containing goldboot.ron
    pub context_dir: PathBuf,

    /// Ephemeral directory for per-run files (SSH keys, FAT images, TPM sockets)
    pub tmp: tempfile::TempDir,

    pub ovmf_path: PathBuf,

    /// Persistent qcow2 path derived from the context directory
    pub qcow_path: PathBuf,

    pub qcow: Option<Qcow3>,

    /// VNC port for the VM
    pub vnc_port: u16,

    /// End time of the run
    pub end_time: Option<SystemTime>,

    /// Start time of the run
    pub start_time: Option<SystemTime>,
}

impl Builder {
    pub fn new(elements: Vec<OsConfig>, context_dir: PathBuf) -> Self {
        let qcow_path = qcow_cache_path(&context_dir).expect("failed to compute qcow cache path");
        let tmp = tempfile::tempdir().unwrap();

        Self {
            accel: detect_accel(),
            debug: false,
            record: false,
            end_time: None,
            qcow: None,
            qcow_path,
            start_time: None,
            vnc_port: rand::rng().random_range(5900..5999),
            elements,
            ovmf_path: tmp.path().join("OVMF.fd"),
            tmp,
            context_dir,
        }
    }

    /// Return true if the working qcow2 already contains a snapshot with the given name.
    pub fn has_checkpoint(&self, name: &str) -> bool {
        self.qcow
            .as_ref()
            .map(|q| q.snapshots.iter().any(|s| s.name == name))
            .unwrap_or(false)
    }

    /// The system architecture
    pub fn arch(&self) -> Result<ImageArch> {
        match self.elements.first() {
            Some(element) => Ok(element.0.os_arch()),
            None => bail!("No elements in builder"),
        }
    }

    /// Run the image build process according to the given command line.
    pub fn run(&mut self, cli: Commands) -> Result<()> {
        self.start_time = Some(SystemTime::now());

        let qcow_size: u64 = self
            .elements
            .iter()
            .map(|element| element.0.os_size())
            .sum();

        match cli {
            Commands::Build {
                record,
                debug,
                read_password,
                no_accel,
                clean,
                output,
                path: _,
                ovmf_path,
            } => {
                self.debug = debug;
                self.record = record;

                // Set VNC port predictably in debug mode
                if debug {
                    self.vnc_port = 5900;
                }

                // Prompt password
                let password = if read_password {
                    Some(
                        Password::with_theme(&crate::cli::cmd::init::theme())
                            .with_prompt("Image encryption passphrase")
                            .interact()?,
                    )
                } else {
                    None
                };

                // Disable VM acceleration if requested
                if no_accel {
                    self.accel = Accel::Tcg;
                }

                // Override from command line
                if let Some(path) = ovmf_path {
                    self.ovmf_path = PathBuf::from(path);
                } else {
                    // Try to find OVMF firmware or unpack what's included
                    if let Some(path) = crate::builder::ovmf::find() {
                        self.ovmf_path = path;
                    } else if cfg!(feature = "include_ovmf") {
                        let path = self
                            .tmp
                            .path()
                            .join("OVMF.fd")
                            .to_string_lossy()
                            .to_string();

                        #[cfg(feature = "include_ovmf")]
                        crate::builder::ovmf::write(self.arch()?, &path).unwrap();
                        self.ovmf_path = PathBuf::from(path);
                    }
                }

                // Check OVMF firmware path
                if !self.ovmf_path.exists() {
                    bail!("No OVMF firmware found");
                }

                if clean && self.qcow_path.exists() {
                    std::fs::remove_file(&self.qcow_path)?;
                }

                if self.qcow_path.exists() {
                    if let Ok(qcow) = Qcow3::open(&self.qcow_path) {
                        if qcow.snapshots.is_empty() {
                            std::fs::remove_file(&self.qcow_path)?;
                        }
                    }
                }

                self.qcow = Some(if self.qcow_path.exists() {
                    Qcow3::open(&self.qcow_path)?
                } else {
                    // Truncate the image size to a power of two for the qcow storage
                    Qcow3::create(&self.qcow_path, qcow_size - (qcow_size % 2))?
                });

                // Revert to the last snapshot if one exists
                if let Some(qcow) = &self.qcow {
                    if let Some(last) = qcow.snapshots.last() {
                        qcow.revert(&last.name)?;
                    }
                }

                for element in self.elements.iter() {
                    element.0.build(&self)?;
                }

                // Re-open qcow to pick up any new snapshots written during the build
                self.qcow = Some(Qcow3::open(&self.qcow_path)?);

                // Convert into final immutable image
                let path = if let Some(output) = output.as_ref() {
                    PathBuf::from(output)
                } else {
                    ImageLibrary::open().temporary()
                };

                let element_headers: Vec<ElementHeader> = self
                    .elements
                    .iter()
                    .map(|e| ElementHeader::new(e.0.os_name(), e.0.os_name()))
                    .collect::<Result<_>>()?;

                ImageHandle::from_qcow(
                    element_headers,
                    self.qcow.as_ref().unwrap(),
                    &path,
                    password,
                    |_, _| {},
                )?;

                if let None = output {
                    ImageLibrary::open().add_move(path.clone())?;
                }
            }
            _ => panic!("Must be passed a Commands::Build"),
        }

        self.end_time = Some(SystemTime::now());
        info!(
            duration = ?self.start_time.unwrap().elapsed()?,
            "Build completed",
        );

        Ok(())
    }
}
