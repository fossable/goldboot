use crate::progress::ProgressBar;
use log::{debug, info};
use sha1::{Digest, Sha1};
use sha2::{Sha256, Sha512};
use simple_error::bail;
use std::{
    error::Error,
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
};

/// Represents the local install media cache.
pub struct MediaCache;

impl MediaCache {
    pub fn get(url: String, checksum: &str) -> Result<String, Box<dyn Error>> {
        let id = hex::encode(Sha1::new().chain_update(&url).finalize());
        let path = cache_dir().join(id);
        std::fs::create_dir_all(cache_dir())?;

        // Delete file if the checksum doesn't match
        if path.is_file() {
            if !verify_checksum(path.to_string_lossy().to_string(), checksum).is_ok() {
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
                let length = rs.content_length().ok_or("Failed to get content length")?;
                let mut file = File::create(&path)?;

                info!("Saving install media");
                ProgressBar::Download.copy(&mut rs, &mut file, length)?;
            } else {
                bail!("Failed to download");
            }

            verify_checksum(path.to_string_lossy().to_string(), checksum)?;
        }

        Ok(path.to_string_lossy().to_string())
    }
}

fn cache_dir() -> PathBuf {
    if cfg!(target_os = "linux") {
        PathBuf::from(format!("/home/{}/.cache/goldboot", whoami::username()))
    } else if cfg!(target_os = "macos") {
        PathBuf::from(format!("/Users/{}/.cache/goldboot", whoami::username()))
    } else {
        panic!("Unsupported platform");
    }
}

fn verify_checksum(path: String, checksum: &str) -> Result<(), Box<dyn Error>> {
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
