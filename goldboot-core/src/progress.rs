use std::{
	cmp::min,
	error::Error,
	io::{Read, Write},
};

pub enum ProgressBar {
	/// A hashing operation
	Hash,

	/// An image conversion operation
	Convert,

	/// A download operation
	Download,

	/// An image write operation
	Write,
}

impl ProgressBar {
	fn create_progressbar(&self, len: u64) -> indicatif::ProgressBar {
		match self {
			ProgressBar::Hash => {
				let progress = indicatif::ProgressBar::new(len);
				progress.set_style(indicatif::ProgressStyle::default_bar().template("{spinner:.blue} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})").progress_chars("=>-"));
				progress
			}
			ProgressBar::Convert => {
				let progress = indicatif::ProgressBar::new(len);
				progress.set_style(indicatif::ProgressStyle::default_bar().template("{spinner:.yellow} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})").progress_chars("=>-"));
				progress
			}
			ProgressBar::Download => {
				let progress = indicatif::ProgressBar::new(len);
				progress.set_style(indicatif::ProgressStyle::default_bar().template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})").progress_chars("=>-"));
				progress
			}
			ProgressBar::Write => {
				let progress = indicatif::ProgressBar::new(len);
				progress.set_style(indicatif::ProgressStyle::default_bar().template("{spinner:.red} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})").progress_chars("=>-"));
				progress
			}
		}
	}

	pub fn new(&self, len: u64) -> Box<dyn Fn(u64)> {
		if !crate::is_interactive() {
			// No progress bar
			return Box::new(|_| {});
		}

		let progress = self.create_progressbar(len);
		Box::new(move |v| {
			if progress.position() + v >= len {
				progress.finish_and_clear();
			} else {
				progress.inc(v);
			}
		})
	}

	/// Fully copy the given reader to the given writer and display a progressbar if
	/// running in interactive mode.
	pub fn copy(
		&self,
		reader: &mut dyn Read,
		writer: &mut dyn Write,
		len: u64,
	) -> Result<(), Box<dyn Error>> {
		if !crate::is_interactive() {
			// No progress bar
			std::io::copy(reader, writer)?;
			return Ok(());
		}

		let progress = self.create_progressbar(len);

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

		progress.finish_and_clear();
		Ok(())
	}
}
