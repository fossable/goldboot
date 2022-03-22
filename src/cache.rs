use bzip2_rs::DecoderReader;
use indicatif::{ProgressBar, ProgressStyle};
use log::{debug, info};
use sha1::{Digest, Sha1};
use sha2::Sha256;
use simple_error::bail;
use std::cmp::min;
use std::error::Error;
use std::fs::File;
use std::io::Read;
use std::io::Write;
use std::path::PathBuf;

fn copy_progress(reader: &mut dyn Read, writer: &mut dyn Write, len: u64, progress: &ProgressBar) -> Result<(), Box<dyn Error>> {
	let mut buffer = [0u8; 1024 * 1024];
    let mut copied: u64 = 0;

    loop {
        if let Ok(size) = reader.read(&mut buffer) {
        	if size == 0 {
        		break
        	}
            writer.write(&buffer[0..size])?;
            let new = min(copied + (size as u64), len);
            copied = new;
            progress.set_position(new);
        } else {
        	break;
        }
    }

    progress.finish();
    Ok(())
}

fn cache_dir() -> PathBuf {
    if cfg!(target_os = "linux") {
        PathBuf::from(format!("/home/{}/.cache/goldboot", whoami::username()))
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
    let file_size = std::fs::metadata(&path)?.len();

    let progress = ProgressBar::new(file_size);
    progress.set_style(ProgressStyle::default_bar().template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})").progress_chars("#>-"));

    let hash = match c[0] {
        "sha1" | "SHA1" => {
        	info!("Computing SHA1 checksum");
            let mut hasher = Sha1::new();
            copy_progress(&mut file, &mut hasher, file_size, &progress)?;
            hex::encode(hasher.finalize())
        }
        "sha256" | "SHA256" => {
        	info!("Computing SHA256 checksum");
            let mut hasher = Sha256::new();
            copy_progress(&mut file, &mut hasher, file_size, &progress)?;
            hex::encode(hasher.finalize())
        }
        _ => bail!("Unsupported hash"),
    };
    debug!("Computed hash: {}", &hash);
    debug!("Expected hash: {}", &c[1]);

    if hash != c[1] {
        bail!("Hash mismatch");
    }

    Ok(())
}

pub struct MediaCache {}

impl MediaCache {
    pub fn get(url: String, checksum: &str) -> Result<String, Box<dyn Error>> {
        let id = hex::encode(Sha1::new().chain_update(&url).finalize());
        let path = cache_dir().join(id);
        std::fs::create_dir_all(cache_dir())?;

        // Delete file if the checksum doesn't match
        if path.is_file() {
            if !verify_checksum(path.to_string_lossy().to_string(), checksum).is_ok() {
            	info!("Cached file checksum did not match");
                std::fs::remove_file(&path)?;
            }
        }

        if !path.is_file() {
        	let mut rs = reqwest::blocking::get(&url)?;
            if rs.status().is_success() {
                let length = rs.content_length().ok_or("Failed to get content length")?;

                // Configure progressbar
                let progress = ProgressBar::new(length);
                progress.set_style(ProgressStyle::default_bar().template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})").progress_chars("=>-"));

                let mut file = File::create(&path)?;

                info!("Saving install media");
                copy_progress(&mut rs, &mut file, length, &progress)?;
            }

            verify_checksum(path.to_string_lossy().to_string(), checksum)?;
        }

        Ok(path.to_string_lossy().to_string())
    }

    pub fn get_bzip2(url: String, checksum: &str) -> Result<String, Box<dyn Error>> {
        let id = hex::encode(Sha1::new().chain_update(&url).finalize());
        let path = cache_dir().join(id);
        std::fs::create_dir_all(cache_dir())?;

        // Delete file if the checksum doesn't match
        if path.is_file() {
            if !verify_checksum(path.to_string_lossy().to_string(), checksum).is_ok() {
            	info!("Cached file checksum did not match");
                std::fs::remove_file(&path)?;
            }
        }

        if !path.is_file() {
            let rs = reqwest::blocking::get(&url)?;
            if rs.status().is_success() {
                let length = rs.content_length().ok_or("Failed to get content length")?;

                // Configure progressbar
                let progress = ProgressBar::new(length);
                progress.set_style(ProgressStyle::default_bar().template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})").progress_chars("=>-"));

                let mut reader = DecoderReader::new(rs);
                let mut file = File::create(&path)?;

                info!("Saving install media");
                copy_progress(&mut reader, &mut file, length, &progress)?;
            }

            verify_checksum(path.to_string_lossy().to_string(), checksum)?;
        }

        Ok(path.to_string_lossy().to_string())
    }
}
