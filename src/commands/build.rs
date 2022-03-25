use crate::commands::image::ImageMetadata;
use crate::qemu::allocate_image;
use crate::{config::Config, image_library_path};
use log::{debug, info};
use simple_error::bail;
use std::{error::Error, fs};

pub fn build() -> Result<(), Box<dyn Error>> {
    println!("⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜");
    println!("⬜　　　　　　　　⬛　　　⬛　⬛　　　　　　　　　　　⬛　⬜");
    println!("⬜　　　　　　　　⬛　　　⬛　⬛　　　　　　　　　　　⬛⬛⬜");
    println!("⬜⬛⬛⬛　⬛⬛⬛　⬛　⬛⬛⬛　⬛⬛⬛　⬛⬛⬛　⬛⬛⬛　⬛　⬜");
    println!("⬜⬛　⬛　⬛　⬛　⬛　⬛　⬛　⬛　⬛　⬛　⬛　⬛　⬛　⬛　⬜");
    println!("⬜⬛⬛⬛　⬛⬛⬛　⬛　⬛⬛⬛　⬛⬛⬛　⬛⬛⬛　⬛⬛⬛　⬛⬛⬜");
    println!("⬜　　⬛　　　　　　　　　　　　　　　　　　　　　　　　　⬜");
    println!("⬜⬛⬛⬛　　　　　　　　　　　　　　　　　　　　　　　　　⬜");
    println!("⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜");

    // Load goldboot.json in the current directory
    let config = Config::load()?;

    // Prepare to build profiles
    let profiles = config.get_profiles();
    let profiles_len = profiles.len();
    if profiles_len == 0 {
        bail!("At least one base profile must be specified");
    }

    // Create an initial image that will be attached as storage to each VM
    let image = allocate_image(&config.disk_size)?;

    // Create partitions if we're multi booting
    if profiles.len() > 1 {
        // TODO
    }

    // Build each profile
    for profile in profiles {
        profile.build(&config, &image)?;
    }

    // Install bootloader if we're multi booting
    if profiles_len > 1 {
        // TODO
    }

    debug!("Build completed successfully");

    // Create new image metadata
    let metadata = ImageMetadata::new(config.clone())?;
    metadata.write()?;

    // Move the image to the library
    fs::rename(
        image_library_path().join("output").join(&config.name),
        metadata.path_qcow2(),
    )
    .unwrap();

    return Ok(());
}
