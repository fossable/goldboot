use crate::image_library_path;
use goldboot_core::*;
use goldboot_image::{levels::ClusterDescriptor, CompressionType};
use log::debug;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
	error::Error,
	fs,
	fs::File,
	io::{BufReader, Seek, SeekFrom, Write},
	path::PathBuf,
	process::Command,
};
use validator::Validate;

impl ImageMetadata {
	pub fn new(config: Config) -> Result<ImageMetadata, Box<dyn Error>> {
		let output = image_library_path().join("output").join(&config.name);
		Ok(ImageMetadata {
			//name: output.file_stem().unwrap().to_str().unwrap().to_string(),
			sha256: "".into(),
			size: fs::metadata(output).unwrap().len(),
			generate_time: 0u64,
			parent_image: "".into(),
			config: config,
		})
	}
}

pub fn info(image: &str) -> Result<(), Box<dyn Error>> {
	Ok(())
}

/// List all images in the image library.
pub fn list() -> Result<(), Box<dyn Error>> {
	let images = ImageMetadata::load()?;

	print!("Image\n");
	for image in images {
		// TODO
	}
	Ok(())
}

pub fn write(image_name: &str, disk_name: &str) -> Result<(), Box<dyn Error>> {
	// TODO backup option

	// Locate the requested image
	let image = ImageMetadata::find(image_name)?;

	// Verify sizes are compatible
	//if image.size != disk.total_space() {
	//    bail!("The requested disk size is not equal to the image size");
	//}

	// Check if mounted
	// TODO

	// Update EFI vars
	// TODO

	let mut f = File::open("foo.txt").unwrap();

	let qcow2 = goldboot_image::GoldbootImage::open(image.path_qcow2())?;
	let mut file = BufReader::new(File::open(image.path_qcow2())?);

	let mut offset = 0u64;
	let mut buffer = [0u8, 1 << qcow2.header.cluster_bits];

	for l1_entry in qcow2.l1_table {
		if l1_entry.l2_offset() != 0 {
			if let Some(l2_table) = l1_entry.read_l2(&mut file, qcow2.header.cluster_bits) {
				for l2_entry in l2_table {
					match &l2_entry.cluster_descriptor {
						ClusterDescriptor::Standard(cluster) => {
							if cluster.host_cluster_offset != 0 {
								debug!("Uncompressed cluster: {:?}", cluster);
								l2_entry
									.read_contents(&mut file, &mut buffer, CompressionType::Zlib)
									.unwrap();
								f.seek(SeekFrom::Start(offset)).unwrap();
								f.write_all(&buffer).unwrap();
							}
						}
						ClusterDescriptor::Compressed(cluster) => {
							debug!("Compressed cluster: {:?}", cluster);
						}
					}
					offset += 1 << qcow2.header.cluster_bits;
				}
			}
		} else {
			offset += u64::pow(1 << qcow2.header.cluster_bits, 2) / 8;
		}
	}
	Ok(())
}

pub fn run(image_name: &str) -> Result<(), Box<dyn Error>> {
	// Locate the requested image
	let image = ImageMetadata::find(image_name)?;

	Command::new("qemu-system-x86_64")
		.args([
			"-display",
			"gtk",
			"-machine",
			"type=pc,accel=kvm",
			"-m",
			"1000M",
			"-boot",
			"once=c",
			"-bios",
			"/usr/share/ovmf/x64/OVMF.fd",
			"-pflash",
			"/tmp/test.fd",
			"-drive",
			&format!(
				"file={},if=virtio,cache=writeback,discard=ignore,format=qcow2",
				image.path_qcow2().display()
			),
			"-name",
			"cli",
		])
		.status()
		.unwrap();
	Ok(())
}
