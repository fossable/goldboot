use self::qemu::{Accel, detect_accel};
use self::{fabricators::Fabricator, os::Os};
use crate::builder::os::BuildImage;
use crate::cli::cmd::Commands;
use crate::library::ImageLibrary;
use crate::size;

use anyhow::{Result, anyhow, bail};
use byte_unit::Byte;
use dialoguer::Password;
use goldboot_image::ElementHeader;
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

/// Machinery that creates Goldboot images from image elements.
#[derive(Validate)]
pub struct Builder {
    pub elements: Vec<Os>,

    pub accel: Accel,

    pub debug: bool,

    pub record: bool,

    /// A general purpose temporary directory for the run
    pub tmp: tempfile::TempDir,

    pub ovmf_path: PathBuf,
    pub qcow_path: PathBuf,

    /// VNC port for the VM
    pub vnc_port: u16,

    /// End time of the run
    pub end_time: Option<SystemTime>,

    /// Start time of the run
    pub start_time: Option<SystemTime>,
}

impl Builder {
    pub fn new(elements: Vec<Os>) -> Self {
        // Allocate directory for the builder to store the intermediate qcow image
        // and any other supporting files.
        let tmp = tempfile::tempdir().unwrap();

        Self {
            accel: detect_accel(),
            debug: false,
            record: false,
            end_time: None,
            qcow_path: tmp.path().join("image.gb.qcow2"),
            start_time: None,
            vnc_port: rand::rng().random_range(5900..5999),
            elements,
            ovmf_path: tmp.path().join("OVMF.fd"),
            tmp,
        }
    }

    /// The system architecture
    pub fn arch(&self) -> Result<ImageArch> {
        match self.elements.first() {
            Some(element) => {
                Ok(match element {
                    Os::AlpineLinux(_) => ImageArch::Amd64,
                    Os::ArchLinux(inner) => inner.arch.0,
                    Os::Debian(inner) => inner.arch.0,
                    Os::Nix(inner) => inner.arch.0,
                    Os::Windows10(inner) => inner.arch.0,
                    Os::Windows11(inner) => inner.arch.0,
                })
            }
            None => bail!("No elements in builder"),
        }
    }

    /// Run the image build process according to the given command line.
    pub fn run(&mut self, cli: Commands) -> Result<()> {
        self.start_time = Some(SystemTime::now());

        let qcow_size: u64 = self
            .elements
            .iter()
            .map(|element| -> u64 { size!(element).into() })
            .sum();

        match cli {
            Commands::Build {
                record,
                debug,
                read_password,
                no_accel,
                output,
                path,
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

                // Truncate the image size to a power of two for the qcow storage
                let qcow = Qcow3::create(&self.qcow_path, qcow_size - (qcow_size % 2))?;
                for element in self.elements.clone().into_iter() {
                    element.build(&self)?;
                }

                // Convert into final immutable image
                let path = if let Some(output) = output.as_ref() {
                    PathBuf::from(output)
                } else {
                    ImageLibrary::open().temporary()
                };

                ImageHandle::from_qcow(Vec::new(), &qcow, &path, password, |_, _| {})?;

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
