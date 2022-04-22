use indicatif::{ProgressBar, ProgressStyle};
use log::{debug, info};
use sha1::{Digest, Sha1};
use sha2::{Sha256, Sha512};
use simple_error::bail;
use std::{
	cmp::min,
	error::Error,
	fs::File,
	io::{Read, Write},
	path::PathBuf,
};

pub enum MediaFormat {
	Iso,
	Bzip2,
}

/// Represents the local install media cache.
pub struct MediaCache;

impl MediaCache {
	pub fn get(url: String, checksum: &str, format: MediaFormat) -> Result<String, Box<dyn Error>> {
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
			let mut rs = reqwest::blocking::get(&url)?;
			if rs.status().is_success() {
				let length = rs.content_length().ok_or("Failed to get content length")?;

				let mut reader: Read = match format {
					Iso => &rs,
					Bzip2 => bzip2_rs::DecoderReader::new(rs),
				};

				let mut file = File::create(&path)?;

				info!("Saving install media");
				if cfg!(feature = "progressbars") {
					// Configure progressbar
					let progress = ProgressBar::new(length);
					progress.set_style(ProgressStyle::default_bar().template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})").progress_chars("=>-"));
					copy_with_progress(&mut reader, &mut file, length, &progress)?;
				} else {
					std::io::copy(&mut reader, &mut file);
				}
			}

			verify_checksum(path.to_string_lossy().to_string(), checksum)?;
		}

		Ok(path.to_string_lossy().to_string())
	}
}

fn copy_with_progress(
	reader: &mut dyn Read,
	writer: &mut dyn Write,
	len: u64,
	progress: &ProgressBar,
) -> Result<(), Box<dyn Error>> {
	let mut buffer = [0u8; 1024 * 1024];
	let mut copied: u64 = 0;

	loop {
		if let Ok(size) = reader.read(&mut buffer) {
			if size == 0 {
				break;
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

	let mut hasher = match c[0] {
		"sha1" | "SHA1" => {
			info!("Computing SHA1 checksum");
			Sha1::new()
		}
		"sha256" | "SHA256" => {
			info!("Computing SHA256 checksum");
			Sha256::new()
		}
		"sha512" | "SHA512" => {
			info!("Computing SHA512 checksum");
			Sha512::new()
		}
		_ => bail!("Unsupported hash"),
	};

	let mut file = File::open(&path)?;

	if cfg!(feature = "progressbars") {
		let file_size = std::fs::metadata(&path)?.len();
		let progress = ProgressBar::new(file_size);
		progress.set_style(ProgressStyle::default_bar().template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})").progress_chars("#>-"));

		copy_with_progress(&mut file, &mut hasher, file_size, &progress)?;
	} else {
		std::io::copy(&mut file, &mut hasher);
	}

	let hash = hex::encode(hasher.finalize());

	debug!("Computed: {}", &hash);
	debug!("Expected: {}", &c[1]);

	if hash != c[1] {
		bail!("Hash mismatch");
	}

	Ok(())
}
