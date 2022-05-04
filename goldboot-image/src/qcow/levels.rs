use binrw::{BinRead, BinReaderExt};
use std::io::*;

/// An entry in an L1 table that can be used to lookup the location of an L2 table
#[derive(BinRead, Debug, Clone)]
pub struct L1Entry(pub u64);

impl L1Entry {
	pub fn l2_offset(&self) -> u64 {
		self.0 & 0x00ff_ffff_ffff_fe00
	}

	pub fn is_used(&self) -> bool {
		self.0 & 0x8000_0000_0000_0000 != 0
	}

	/// Reads the L2 table corresponding to this L1 entry from the given file
	pub fn read_l2(
		&self,
		reader: &mut (impl Read + Seek),
		cluster_bits: u32,
	) -> Option<Vec<L2Entry>> {
		reader.seek(SeekFrom::Start(self.l2_offset())).ok()?;
		let L2Entries(entries) = reader.read_be_args((cluster_bits,)).ok()?;

		Some(
			entries
				.into_iter()
				.map(|x| L2Entry::from_u64(x, cluster_bits))
				.collect(),
		)
	}
}

#[derive(BinRead)]
#[br(import(cluster_bits: u32))]
struct L2Entries(#[br(count = (1 << cluster_bits) / 8)] Vec<u64>);

/// An entry in an L2 table that can be used to lookup the location and properties of the cluster
#[derive(Debug, Clone)]
pub struct L2Entry {
	/// A descriptor providing the information needed to read from the given cluster
	pub cluster_descriptor: ClusterDescriptor,

	/// true if the contents of the cluster are compressed using a method specified by
	/// the field [`Version3Header::compression_type`], otherwise defaulting to
	/// [`CompressionType::Zlib`], which can also be specified via [`CompressionType::default`].
	pub is_compressed: bool,

	/// Set to false for clusters that are unused, compressed or require COW.
	/// Set to true for standard clusters whose refcount is exactly one.
	/// This information is only accurate in L2 tables that are reachable
	/// from the active L1 table.
	///
	/// With external data files, all guest clusters have an
	/// implicit refcount of 1 (because of the fixed host = guest
	/// mapping for guest cluster offsets), so this bit should be 1
	/// for all allocated clusters.
	pub is_used: bool,
}

impl L2Entry {
	fn from_u64(x: u64, cluster_bits: u32) -> Self {
		let is_compressed = x & 0x4000_0000_0000_0000 != 0;
		L2Entry {
			cluster_descriptor: ClusterDescriptor::from_u64(
				is_compressed,
				x & 0x3fffffffffffffff,
				cluster_bits,
			),
			is_used: x & 0x8000_0000_0000_0000 != 0,
			is_compressed,
		}
	}

	/// Read the contents of a given L2 Entry from `reader` into `buf`.
	pub fn read_contents(
		&self,
		reader: &mut (impl Read + Seek),
		buf: &mut [u8],
	) -> std::io::Result<()> {
		match &self.cluster_descriptor {
			ClusterDescriptor::Standard(cluster) => {
				if cluster.all_zeroes || cluster.host_cluster_offset == 0 {
					buf.fill(0);
				} else {
					reader
						.seek(SeekFrom::Start(cluster.host_cluster_offset))
						.map_err(|_| {
							std::io::Error::new(
								std::io::ErrorKind::UnexpectedEof,
								"Seeked past the end of the file attempting to read the current \
                            cluster",
							)
						})?;

					std::io::copy(
						&mut reader.take(buf.len() as u64),
						&mut std::io::Cursor::new(buf),
					)?;
				}
			}
			ClusterDescriptor::Compressed(cluster) => {
				reader
					.seek(SeekFrom::Start(cluster.host_cluster_offset))
					.map_err(|_| {
						std::io::Error::new(
							std::io::ErrorKind::UnexpectedEof,
							"Seeked past the end of the file attempting to read the current \
                            cluster",
						)
					})?;

				let cluster_size = buf.len() as u64;
				let mut cluster = std::io::Cursor::new(buf);
				std::io::copy(
					&mut zstd::Decoder::new(reader)?.take(cluster_size),
					&mut cluster,
				)?;
			}
		}

		Ok(())
	}
}

/// A descriptor providing the information needed to read from the given cluster regardless of
/// whether or not the cluster itself is compressed.
#[derive(Debug, Clone)]
pub enum ClusterDescriptor {
	/// A descriptor describing an uncompressed cluster
	Standard(StandardClusterDescriptor),

	/// A descriptor describing a compressed cluster
	Compressed(CompressedClusterDescriptor),
}

/// A descriptor describing an uncompressed cluster
#[derive(Debug, Clone)]
pub struct StandardClusterDescriptor {
	/// If set to true, the cluster reads as all zeros. The host
	/// cluster offset can be used to describe a preallocation,
	/// but it won't be used for reading data from this cluster,
	/// nor is data read from the backing file if the cluster is
	/// unallocated.
	///
	/// With version 2 or with extended L2 entries (see the next
	/// section), this is always false.
	pub all_zeroes: bool,

	/// The offset of the cluster within the host file. Must be aligned
	/// to a cluster boundary. If the offset is 0 and [`L2Entry::is_used`]
	/// is clear, the cluster is unallocated. The offset may only be 0 with
	/// [`L2Entry::is_used`] set (indicating a host cluster offset of 0) when an
	/// external data file is used.
	pub host_cluster_offset: u64,
}

impl StandardClusterDescriptor {
	fn from_u64(x: u64) -> Self {
		Self {
			all_zeroes: x & 1 != 0,
			host_cluster_offset: (x & 0x00ff_ffff_ffff_fe00),
		}
	}
}

/// A descriptor describing a compressed cluster
#[derive(Debug, Clone)]
pub struct CompressedClusterDescriptor {
	/// Host cluster offset. This is usually _not_ aligned to a
	/// cluster or sector boundary!  If cluster_bits is
	/// small enough that this field includes bits beyond
	/// 55, those upper bits must be set to 0.
	pub host_cluster_offset: u64,

	/// Number of additional 512-byte sectors used for the
	/// compressed data, beyond the sector containing the offset
	/// in the previous field. Some of these sectors may reside
	/// in the next contiguous host cluster.
	///
	/// Note that the compressed data does not necessarily occupy
	/// all of the bytes in the final sector; rather, decompression
	/// stops when it has produced a cluster of data.
	///
	/// Another compressed cluster may map to the tail of the final
	/// sector used by this compressed cluster.
	pub additional_sector_count: u64,
}

fn mask(bits: u32) -> u64 {
	(1 << bits) - 1
}

impl CompressedClusterDescriptor {
	fn from_u64(x: u64, cluster_bits: u32) -> Self {
		let host_cluster_bits = 62 - (cluster_bits - 8);
		Self {
			host_cluster_offset: x & mask(host_cluster_bits),
			additional_sector_count: (x & !mask(host_cluster_bits)) >> host_cluster_bits,
		}
	}
}

impl ClusterDescriptor {
	fn from_u64(is_compressed: bool, x: u64, cluster_bits: u32) -> Self {
		if is_compressed {
			Self::Compressed(CompressedClusterDescriptor::from_u64(x, cluster_bits))
		} else {
			Self::Standard(StandardClusterDescriptor::from_u64(x))
		}
	}
}
