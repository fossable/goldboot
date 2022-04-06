use crate::config::Config;
use crate::image_library_path;
use log::debug;
use gb_image::levels::ClusterDescriptor;
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

/// Represents a local image.
#[derive(Clone, Serialize, Deserialize, Validate)]
pub struct ImageMetadata {
    pub sha256: String,

    /// The image size in bytes
    pub size: u64,

    pub generate_time: u64,

    pub parent_image: String,

    pub config: Config,
}

impl ImageMetadata {
    /// Write the image metadata to the library and return the metadata hash
    pub fn write(&self) -> Result<(), Box<dyn Error>> {
        let metadata_json = serde_json::to_string(&self).unwrap();

        // Write it to the library directory
        fs::write(self.path_json(), &metadata_json).unwrap();
        Ok(())
    }

    pub fn path_json(&self) -> PathBuf {
        let metadata_json = serde_json::to_string(&self).unwrap();
        let hash = hex::encode(Sha256::new().chain_update(&metadata_json).finalize());

        image_library_path().join(format!("{}.json", hash))
    }

    pub fn path_qcow2(&self) -> PathBuf {
        let metadata_json = serde_json::to_string(&self).unwrap();
        let hash = hex::encode(Sha256::new().chain_update(&metadata_json).finalize());

        image_library_path().join(format!("{}.qcow2", hash))
    }

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

    /// Load images present in the local image library
    pub fn load() -> Result<Vec<ImageMetadata>, Box<dyn Error>> {
        let mut images = Vec::new();

        for p in image_library_path().read_dir().unwrap() {
            let path = p.unwrap().path();

            if let Some(ext) = path.extension() {
                let filename = path.file_stem().unwrap().to_str().unwrap().to_string();
                if ext == "json" {
                    // Hash the file and compare it to the filename
                    let content = fs::read(&path).unwrap();

                    if *Sha256::new().chain_update(content).finalize()
                        == hex::decode(filename).unwrap()
                    {
                        let metadata: ImageMetadata =
                            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
                        images.push(metadata);
                    } else {
                        debug!("Found corrupt file in image directory: {}", path.display());
                    }
                }
            }
        }

        Ok(images)
    }

    pub fn find(image_name: &str) -> Result<ImageMetadata, Box<dyn Error>> {
        Ok(ImageMetadata::load()?
            .iter()
            .find(|&metadata| metadata.config.name == image_name)
            .unwrap()
            .to_owned())
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

    let qcow2 = gb_image::open(image.path_qcow2())?;
    let mut file = BufReader::new(File::open(image.path_qcow2())?);

    let mut offset = 0u64;
    let mut buffer = [0u8, 1 << qcow2.header.cluster_bits];

    for l1_entry in qcow2.l1_table {
        if l1_entry.l2_offset != 0 {
            if let Some(l2_table) = l1_entry.read_l2(&mut file, qcow2.header.cluster_bits) {
                for l2_entry in l2_table {
                    match &l2_entry.cluster_descriptor {
                        ClusterDescriptor::Standard(cluster) => {
                            if cluster.host_cluster_offset != 0 {
                                debug!("Uncompressed cluster: {:?}", cluster);
                                l2_entry
                                    .read_contents(&mut file, &mut buffer, gb_image::CompressionType::Zlib)
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
