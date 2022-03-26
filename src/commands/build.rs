use crate::commands::image::ImageMetadata;
use crate::config::Config;
use log::{debug, info};
use simple_error::bail;
use std::time::Instant;
use std::{error::Error, fs};

pub fn build(record: bool, debug: bool) -> Result<(), Box<dyn Error>> {
    println!("⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜");
    println!("⬜　　　　　　　　⬛　　　⬛　⬛　　　　　　　　　　　⬛　⬜");
    println!("⬜　　　　　　　　⬛　　　⬛　⬛　　　　　　　　　　　⬛⬛⬜");
    println!("⬜⬛⬛⬛　⬛⬛⬛　⬛　⬛⬛⬛　⬛⬛⬛　⬛⬛⬛　⬛⬛⬛　⬛　⬜");
    println!("⬜⬛　⬛　⬛　⬛　⬛　⬛　⬛　⬛　⬛　⬛　⬛　⬛　⬛　⬛　⬜");
    println!("⬜⬛⬛⬛　⬛⬛⬛　⬛　⬛⬛⬛　⬛⬛⬛　⬛⬛⬛　⬛⬛⬛　⬛⬛⬜");
    println!("⬜　　⬛　　　　　　　　　　　　　　　　　　　　　　　　　⬜");
    println!("⬜⬛⬛⬛　　　　　　　　　　　　　　　　　　　　　　　　　⬜");
    println!("⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜");

    let start_time = Instant::now();

    // Load goldboot.json in the current directory
    let config = Config::load()?;

    // Prepare to build profiles
    let profiles = config.get_profiles();
    let profiles_len = profiles.len();
    if profiles_len == 0 {
        bail!("At least one base profile must be specified");
    }

    // Create an initial image that will be attached as storage to each VM
    let image_path = crate::qemu::allocate_image(&config.disk_size)?;

    // Create partitions if we're multi booting
    if profiles.len() > 1 {
        // TODO
    }

    // Build each profile
    for profile in profiles {
        profile.build(&config, &image_path, record, debug)?;
    }

    // Install bootloader if we're multi booting
    if profiles_len > 1 {
        // TODO
    }

    // Attempt to reduce the size of image
    crate::qemu::compact_qcow2(&image_path)?;

    info!("Build completed in: {:?}", start_time.elapsed());

    // Create new image metadata
    let metadata = ImageMetadata::new(config.clone())?;
    metadata.write()?;

    // Move the image to the library
    fs::rename(image_path, metadata.path_qcow2())?;

    Ok(())
}
