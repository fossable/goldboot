use std::cmp::min;
use std::io::Write;
use std::fs::File;
use std::path::PathBuf;
use std::error::Error;
use bzip2_rs::DecoderReader;
use sha1::{Digest, Sha1};
use indicatif::{ProgressBar, ProgressStyle};
use futures_util::StreamExt;
use futures::executor::block_on;

fn cache_dir() -> PathBuf {
	if cfg!(target_os = "linux") {
        PathBuf::from(format!(
            "/home/{}/.cache/goldboot",
            whoami::username()
        ))
    } else {
        panic!("Unsupported platform");
    }
}

pub struct MediaCache {
}

impl MediaCache {

	pub fn get(url: String, checksum: String) -> Result<String, Box<dyn Error>> {
		let id = hex::encode(Sha1::new().chain_update(&url).finalize());
		let path = cache_dir().join(id);
		if ! path.is_file() {

			let mut file = File::create(&path).or(Err(format!("Failed to create file '{}'", path.to_string_lossy())))?;
			let mut downloaded: u64 = 0;

			let rs = block_on(reqwest::get(&url))?;
			let length = rs.content_length().ok_or("Failed to get content length")?;
			let mut stream = rs.bytes_stream();

	        let progress = ProgressBar::new(length);

		    while let Some(item) = block_on(stream.next()) {
		        let chunk = item.or(Err(format!("Error while downloading file")))?;
		        file.write_all(&chunk)
		            .or(Err(format!("Error while writing to file")))?;
		        let new = min(downloaded + (chunk.len() as u64), length);
		        downloaded = new;
		        progress.set_position(new);
		    }

		    //progress.finish_with_message(&format!("Downloaded {}", url));
	    }

        Ok(path.to_string_lossy().to_string())
	}

	pub fn get_bzip2(url: String, checksum: String) -> Result<String, Box<dyn Error>> {
		let id = hex::encode(Sha1::new().chain_update(&url).finalize());
		let file = cache_dir().join(id);
		if ! file.is_file() {

			let rs = reqwest::blocking::get(url)?;
	        if rs.status().is_success() {
	            let mut reader = DecoderReader::new(rs);
	            std::io::copy(&mut reader, &mut File::open(&file)?)?;
	        }
	    }

        Ok(file.to_string_lossy().to_string())
	}
}