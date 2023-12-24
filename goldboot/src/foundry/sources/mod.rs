use crate::cli::progress::ProgressBar;
use anyhow::anyhow;
use anyhow::bail;
use anyhow::Result;
use log::{debug, info};
use serde::Deserialize;
use serde::Serialize;
use sha1::{Digest, Sha1};
use sha2::{Sha256, Sha512};
use std::{
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub mod iso;

pub trait LoadSource {}

/// All builds start with a single `Source` which provides the initial image
/// to be subjected to further customizations.
#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub enum ImageSource {
    Iso {
        url: String,
        checksum: Option<String>,
    },
    Mold {
        base: String,
    },
    #[default]
    Buildroot,
}

/// Simple cache for source installation media like ISOs.
pub struct SourceCache {
    /// Cache location on disk
    pub directory: PathBuf,
}

impl SourceCache {
    /// Get the default platform-dependent source cache.
    pub fn default() -> Result<Self> {
        let directory = if cfg!(target_os = "linux") {
            PathBuf::from(format!(
                "/home/{}/.cache/goldboot/sources",
                whoami::username()
            ))
        } else if cfg!(target_os = "macos") {
            PathBuf::from(format!(
                "/Users/{}/.cache/goldboot/sources",
                whoami::username()
            ))
        } else if cfg!(target_os = "windows") {
            PathBuf::from(format!(
                "C:/Users/{}/AppData/Local/goldboot/cache/sources",
                whoami::username()
            ))
        } else {
            bail!("Unsupported platform");
        };

        // Make sure it exists before we return
        std::fs::create_dir_all(&directory)?;
        Ok(Self { directory })
    }

    pub fn get(&self, url: String, checksum: &str) -> Result<String> {
        let id = hex::encode(Sha1::new().chain_update(&url).finalize());
        let path = self.directory.join(id);

        // Delete file if the checksum doesn't match
        if path.is_file() {
            if !Self::verify_checksum(path.to_string_lossy().to_string(), checksum).is_ok() {
                info!("Deleting corrupt cached file");
                std::fs::remove_file(&path)?;
            }
        }

        if !path.is_file() {
            // Check for local URL
            if !url.starts_with("http") && Path::new(&url).is_file() {
                return Ok(url);
            }

            // Try to download it
            let rs = reqwest::blocking::get(&url)?;
            if rs.status().is_success() {
                let length = rs
                    .content_length()
                    .ok_or_else(|| anyhow!("Failed to get content length"))?;
                let mut file = File::create(&path)?;

                info!("Saving install media");
                ProgressBar::Download.copy(&mut rs, &mut file, length)?;
            } else {
                bail!("Failed to download");
            }

            Self::verify_checksum(path.to_string_lossy().to_string(), checksum)?;
        }

        Ok(path.to_string_lossy().to_string())
    }

    fn verify_checksum(path: String, checksum: &str) -> Result<()> {
        // "None" shortcut
        if checksum == "none" {
            return Ok(());
        }

        let c: Vec<&str> = checksum.split(":").collect();
        if c.len() != 2 {
            bail!("Invalid checksum: {}", checksum);
        }

        let mut file = File::open(&path)?;

        let hash = match c[0] {
            "sha1" | "SHA1" => {
                info!("Computing SHA1 checksum");
                let mut hasher = Sha1::new();
                ProgressBar::Hash.copy(&mut file, &mut hasher, std::fs::metadata(&path)?.len())?;
                hex::encode(hasher.finalize())
            }
            "sha256" | "SHA256" => {
                info!("Computing SHA256 checksum");
                let mut hasher = Sha256::new();
                ProgressBar::Hash.copy(&mut file, &mut hasher, std::fs::metadata(&path)?.len())?;
                hex::encode(hasher.finalize())
            }
            "sha512" | "SHA512" => {
                info!("Computing SHA512 checksum");
                let mut hasher = Sha512::new();
                ProgressBar::Hash.copy(&mut file, &mut hasher, std::fs::metadata(&path)?.len())?;
                hex::encode(hasher.finalize())
            }
            _ => bail!("Unsupported hash"),
        };

        debug!("Computed: {}", &hash);
        debug!("Expected: {}", &c[1]);

        if hash != c[1] {
            bail!("Hash mismatch");
        }

        Ok(())
    }
}
