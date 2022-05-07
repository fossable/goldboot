use binrw::{io::SeekFrom, BinRead, BinReaderExt};
use std::{error::Error, fs::File, io::BufReader, path::Path, process::Command};

mod header;
pub use header::*;

pub mod levels;
use levels::*;

/// Represents a (stripped down) qcow3 file on disk.
#[derive(BinRead, Debug)]
#[brw(big)]
pub struct Qcow3 {
	/// The image header
	pub header: QcowHeader,

	/// The "active" L1 table
	#[br(seek_before = SeekFrom::Start(header.l1_table_offset), count = header.l1_size)]
	pub l1_table: Vec<L1Entry>,

	/// The file path
	#[br(ignore)]
	pub file: String,
}

impl Qcow3 {
	/// Open a qcow3 file from the given path.
	pub fn open(path: impl AsRef<Path>) -> Result<Self, Box<dyn Error>> {
		let mut file = BufReader::new(File::open(&path)?);

		let mut qcow: Qcow3 = file.read_be()?;
		qcow.file = path.as_ref().to_string_lossy().to_string();
		Ok(qcow)
	}

	/// Allocate a new qcow3 file.
	pub fn create(path: &str, size: u64) -> Result<Self, Box<dyn Error>> {
		Command::new("qemu-img")
			.args(["create", "-f", "qcow2", &path, &format!("{size}")])
			.status()
			.unwrap();

		Qcow3::open(path)
	}

	/// Count the number of allocated clusters.
	pub fn count_clusters(&self) -> Result<u16, Box<dyn Error>> {
		let mut count = 0u16;

		for l1_entry in &self.l1_table {
			if let Some(l2_table) =
				l1_entry.read_l2(&mut File::open(&self.file)?, self.header.cluster_bits)
			{
				for l2_entry in l2_table {
					if l2_entry.is_used {
						count += 1;
					}
				}
			}
		}
		Ok(count)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_load() -> Result<(), Box<dyn Error>> {
		Qcow3::open("test/empty.gb")?;
		Ok(())
	}
}
