#![warn(missing_docs)]
use binrw::{
    io::{Read, Seek, SeekFrom},
    until_exclusive, BinRead, BinReaderExt, BinWrite,
};
use modular_bitfield::prelude::*;

use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

mod reader;
pub use reader::*;

mod writer;
pub use writer::*;

mod header;
pub use header::*;

pub mod levels;
use levels::*;

mod snapshots;
pub use snapshots::*;

/// Parsed representation of a qcow2 file.
///
/// Can be aquired by using one of:
///
/// * [`open`]
/// * [`load`]
/// * [`load_from_memory`]
///
/// and then using [`DynamicQcow::unwrap_qcow2`].
#[derive(BinRead, BinWrite, Debug)]
#[brw(big)]
pub struct GoldbootImage {
    /// Header of the qcow as parsed from the file, contains top-level data about the qcow
    pub header: QcowHeader,

    /// List of snapshots present within this qcow
    #[br(seek_before = SeekFrom::Start(header.snapshots_offset), count = header.nb_snapshots)]
    #[bw(seek_before = SeekFrom::Start(header.snapshots_offset))]
    pub snapshots: Vec<Snapshot>,

    /// Active table of [`L1Entry`]s used for handling lookups of contents
    #[br(seek_before = SeekFrom::Start(header.l1_table_offset), count = header.l1_size)]
    #[bw(seek_before = SeekFrom::Start(header.l1_table_offset))]
    pub l1_table: Vec<L1Entry>,
}

impl GoldbootImage {
    /// Get the size of a cluster in bytes from the qcow
    pub fn cluster_size(&self) -> u64 {
        self.header.cluster_size()
    }

    /// Open a qcow or qcow2 file from a path
    pub fn open(path: impl AsRef<Path>) -> Result<GoldbootImage, Box<dyn Error>> {
        let path = path.as_ref();
        let mut file = BufReader::new(File::open(path)?);

        GoldbootImage::load(&mut file)
    }

    pub fn new(size: u64, metadata: Vec<u8>) -> GoldbootImage {
        let header = QcowHeader::new(size, metadata);
        let l1_size = header.l1_size;

        GoldbootImage {
            header: header,
            snapshots: vec![],
            l1_table: vec![L1Entry(0); l1_size as usize],
        }
    }

    pub fn create(
        path: impl AsRef<Path>,
        size: u64,
        metadata: Vec<u8>,
    ) -> Result<GoldbootImage, Box<dyn Error>> {
        let image = GoldbootImage::new(size, metadata);
        image.write_to(&mut File::create(path)?)?;
        Ok(image)
    }

    /// Read a qcow or qcow2 file from a reader
    ///
    /// **Note**: unlike [`open`] this does not buffer your I/O. Any buffering should be handled via a
    /// wrapper such as [`BufReader`] in order to ensure good performance where applicable.
    pub fn load(reader: &mut (impl Read + Seek)) -> Result<GoldbootImage, Box<dyn Error>> {
        Ok(reader.read_be()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use binrw::BinWrite;

    #[test]
    fn test_load() -> Result<(), Box<dyn Error>> {
        GoldbootImage::load(&mut File::open("test/empty.gb")?)?;
        Ok(())
    }

    #[test]
    fn test_new() -> Result<(), Box<dyn Error>> {
        let image = GoldbootImage::new(5120000000, vec![]);
        image.write_to(&mut File::create("test.gb")?)?;
        Ok(())
    }
}
