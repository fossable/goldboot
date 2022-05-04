use binrw::{io::SeekFrom, BinRead, BinReaderExt};

use std::{error::Error, fs::File, io::BufReader, path::Path};

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

	#[br(ignore)]
	pub file: Option<BufReader<File>>,
}

impl Qcow3 {
	/// Open a qcow3 file from the given path
	pub fn open(path: impl AsRef<Path>) -> Result<Self, Box<dyn Error>> {
		let path = path.as_ref();
		let mut file = BufReader::new(File::open(path)?);

		let mut qcow: Qcow3 = file.read_be()?;
		qcow.file = Some(file);
		Ok(qcow)
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
