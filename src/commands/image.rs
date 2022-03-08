use std::{
    path::{Path, PathBuf},
    fs,
    io::{Write, Seek, SeekFrom, BufReader},
    fs::File,
    process::Command,
};
use crate::image_library_path;
use anyhow::Result;
use log::debug;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tabled::Tabled;
use validator::Validate;
use qcow::levels::ClusterDescriptor;
use qcow::*;

/// Represents a local image.
#[derive(Clone, Serialize, Deserialize, Validate, Tabled)]
pub struct ImageMetadata {
    pub name: String,

    pub sha256: String,

    /// The image size in bytes
    pub size: u64,

    pub generate_time: u64,

    pub parent_image: String,
}

impl ImageMetadata {
    /// Write the image metadata to the library and return the metadata hash
    pub fn write(&self) -> Result<String> {
        let metadata_json = serde_json::to_string(&self).unwrap();
        let hash = hex::encode(Sha256::new().chain_update(&metadata_json).finalize());

        // Write it to the library directory
        fs::write(
            image_library_path().join(format!("{}.json", hash)),
            &metadata_json,
        )
        .unwrap();
        Ok(hash)
    }

    pub fn path(&self) -> &Path {
        &Path::new("")
    }

    pub fn new(output: PathBuf) -> Result<ImageMetadata> {
        Ok(ImageMetadata {
            name: output.file_stem().unwrap().to_str().unwrap().to_string(),
            sha256: "".into(),
            size: fs::metadata(output).unwrap().len(),
            generate_time: 0u64,
            parent_image: "".into(),
        })
    }

    /// Load images present in the local image library
    pub fn load() -> Result<Vec<ImageMetadata>> {
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

    pub fn find(image_name: &str) -> Result<ImageMetadata> {
        Ok(ImageMetadata::load()?
            .iter()
            .find(|&metadata| metadata.name == image_name)
            .unwrap()
            .to_owned())
    }
}

/// List all images in the image library.
pub fn list() -> Result<()> {
    let images = ImageMetadata::load()?;

    print!("{}", Table::new(images).with(Style::modern()).to_string());
    Ok(())
}

pub fn write(image_name: &str, disk_name: &str) -> Result<()> {
    // TODO backup option

    // Locate the requested image
    let image = ImageMetadata::find(image_name)?;

    // Locate the requested disk
    debug!("disks: {:?}", System::new_with_specifics(RefreshKind::new().with_disks_list()).disks());
    if let Some(disk) = System::new_with_specifics(RefreshKind::new().with_disks_list())
        .disks()
        .iter()
        .find(|&d| d.name() == disk_name)
    {
        debug!("Found disk: {:?}", disk);

        // Verify sizes are compatible
        if image.size != disk.total_space() {
            bail!("The requested disk size is not equal to the image size");
        }

        // Check if mounted
        // TODO

        let mut f = File::open("foo.txt").unwrap();

        let qcow2 = qcow::open("/var/lib/goldboot/images/cd019f625eba2fc001b116065485ef0ed9ed33a1fa34bcc5584acd5b88a6d4f0.qcow2").unwrap().unwrap_qcow2();
        let mut file = BufReader::new(File::open("/var/lib/goldboot/images/cd019f625eba2fc001b116065485ef0ed9ed33a1fa34bcc5584acd5b88a6d4f0.qcow2").unwrap());

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
                                    l2_entry.read_contents(&mut file, &mut buffer, CompressionType::Zlib).unwrap();
                                    f.seek(SeekFrom::Start(offset)).unwrap();
                                    f.write_all(&buffer).unwrap();
                                }
                            },
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
    } else {
        bail!("Disk not found: {}", disk_name);
    }
    Ok(())
}

pub fn run(image: &str) -> Result<()> {
    Command::new("qemu-system-x86_64").args([
        "-display",
        "gtk",
        "-machine",
        "type=pc,accel=kvm",
        "-m",
        "1000M",
        "-boot",
        "once=d",
        "-drive",
        "file=/var/lib/goldboot/images/da1d9c276e89c1a7cdc27fe6b52b1449e6d0feb9c7f9ac38873210f4359f0642,if=virtio,cache=writeback,discard=ignore,format=qcow2",
        "-name",
        "cli",
    ])
    .status().unwrap();
    Ok(())
}