use self::qemu::{Accel, detect_accel};
use crate::builder::os::OsConfig;
use crate::cli::cmd::Commands;
use crate::library::{ImageLibrary, qcow_cache_path};

use anyhow::{Result, bail};
use chrono::Utc;
use dialoguer::Password;
use goldboot_image::{
    ElementHeader, ImageArch, ImageHandle, ImageRef, qcow::Qcow3, validate_ref_segment,
};
use rand::RngExt;
use std::{
    path::{Path, PathBuf},
    time::SystemTime,
};
use tracing::info;
use validator::Validate;

pub mod config;
pub mod http;
pub mod options;
pub mod os;
pub mod ovmf;
pub mod qemu;
pub mod sources;
pub mod ssh;
pub mod steps;
pub mod vnc;

/// Machinery that creates Goldboot images from image elements.
#[derive(Validate)]
pub struct Builder {
    /// Image name from `goldboot.ron` (written into `PrimaryHeader.name`).
    pub name: String,

    pub elements: Vec<OsConfig>,

    pub accel: Accel,

    pub debug: bool,

    pub record: bool,

    /// Context directory containing goldboot.ron
    pub context_dir: PathBuf,

    /// Directory the build reads config files from. Normally `context_dir`,
    /// but when pre-steps are present this is an ephemeral copy of it that
    /// the steps are free to modify.
    pub effective_context_dir: PathBuf,

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
    pub fn new(name: String, elements: Vec<OsConfig>, context_dir: PathBuf) -> Self {
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
            name,
            elements,
            ovmf_path: tmp.path().join("OVMF.fd"),
            tmp,
            effective_context_dir: context_dir.clone(),
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

        let minimum_qcow_size: u64 = self
            .elements
            .iter()
            .map(|element| element.0.os_minimum_size())
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
                tag,
                name: _,
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
                    self.ovmf_path = path;
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
                    // Truncate the minimum size to a power of two for the qcow storage
                    Qcow3::create(&self.qcow_path, minimum_qcow_size - (minimum_qcow_size % 2))?
                });

                // Revert to the last snapshot if one exists
                if let Some(qcow) = &self.qcow {
                    if let Some(last) = qcow.snapshots.last() {
                        qcow.revert(&last.name)?;
                    }
                }

                // Pre-steps may modify the context directory, so give them an
                // ephemeral copy and read config files from it for the rest
                // of the build.
                if self
                    .elements
                    .iter()
                    .any(|element| !element.0.pre_steps().is_empty())
                {
                    let copy = self.tmp.path().join("context");
                    copy_dir_all(&self.context_dir, &copy)?;
                    self.effective_context_dir = copy;

                    for element in self.elements.iter() {
                        for pre_step in element.0.pre_steps() {
                            pre_step.run(&self.effective_context_dir)?;
                        }
                    }
                }

                for element in self.elements.iter() {
                    element.0.build(self)?;
                }

                // Re-open qcow to pick up any new snapshots written during the build
                self.qcow = Some(Qcow3::open(&self.qcow_path)?);

                // Resolve the image tag: CLI override → timestamp default.
                let resolved_tag = tag
                    .clone()
                    .unwrap_or_else(|| Utc::now().format("%Y%m%dT%H%M%S").to_string());
                validate_ref_segment(&resolved_tag)?;

                // Convert into final immutable image. When --output is given
                // the file lands at the user-supplied path and the library
                // is not touched; otherwise we stage to a temp path inside
                // the library, then promote to <library>/local/<name>/<tag>.gb.
                let library = ImageLibrary::open();
                let path = if let Some(output) = output.as_ref() {
                    PathBuf::from(output)
                } else {
                    library.temporary()
                };

                let element_headers: Vec<ElementHeader> = self
                    .elements
                    .iter()
                    .map(|e| ElementHeader::new(e.0.os_name(), e.0.os_name()))
                    .collect::<Result<_>>()?;

                ImageHandle::from_qcow(
                    &self.name,
                    &resolved_tag,
                    element_headers,
                    self.qcow.as_ref().unwrap(),
                    &path,
                    password,
                    |_, _| {},
                )?;

                if output.is_none() {
                    // Freshly-built images have no host — they live directly
                    // under the library root until pushed.
                    let built_ref = ImageRef::new(&self.name).with_tag(&resolved_tag);
                    library.add_built(&path, &built_ref)?;
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

/// Recursively copy a directory, skipping `.git`.
fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        if entry.file_name() == ".git" {
            continue;
        }
        let dst_path = dst.join(entry.file_name());
        // `is_dir` follows symlinks, so a symlinked directory is deep-copied
        if entry.path().is_dir() {
            copy_dir_all(&entry.path(), &dst_path)?;
        } else {
            std::fs::copy(entry.path(), &dst_path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_dir_all_copies_recursively_and_skips_git() -> Result<()> {
        let src = tempfile::tempdir()?;
        std::fs::write(src.path().join("goldboot.ron"), "config")?;
        std::fs::create_dir_all(src.path().join("nested"))?;
        std::fs::write(src.path().join("nested/file"), "nested")?;
        std::fs::create_dir_all(src.path().join(".git"))?;
        std::fs::write(src.path().join(".git/HEAD"), "ref")?;

        let dst = tempfile::tempdir()?;
        let dst = dst.path().join("context");
        copy_dir_all(src.path(), &dst)?;

        assert_eq!(std::fs::read_to_string(dst.join("goldboot.ron"))?, "config");
        assert_eq!(std::fs::read_to_string(dst.join("nested/file"))?, "nested");
        assert!(!dst.join(".git").exists());

        // The original is untouched by modifications to the copy
        std::fs::write(dst.join("goldboot.ron"), "modified")?;
        assert_eq!(
            std::fs::read_to_string(src.path().join("goldboot.ron"))?,
            "config"
        );
        Ok(())
    }
}
