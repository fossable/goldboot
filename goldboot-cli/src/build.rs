use colored::*;
use goldboot_core::*;
use log::{debug, info};
use simple_error::bail;
use std::time::Instant;
use std::{error::Error, fs};

#[rustfmt::skip]
fn print_banner() {
    println!("⬜{}⬜", "⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜");
    println!("⬜{}⬜", "　　　　　　　　⬛　　　⬛　⬛　　　　　　　　　　　⬛　".truecolor(200, 171, 55));
    println!("⬜{}⬜", "　　　　　　　　⬛　　　⬛　⬛　　　　　　　　　　　⬛⬛".truecolor(200, 171, 55));
    println!("⬜{}⬜", "⬛⬛⬛　⬛⬛⬛　⬛　⬛⬛⬛　⬛⬛⬛　⬛⬛⬛　⬛⬛⬛　⬛　".truecolor(200, 171, 55));
    println!("⬜{}⬜", "⬛　⬛　⬛　⬛　⬛　⬛　⬛　⬛　⬛　⬛　⬛　⬛　⬛　⬛　".truecolor(200, 171, 55));
    println!("⬜{}⬜", "⬛⬛⬛　⬛⬛⬛　⬛　⬛⬛⬛　⬛⬛⬛　⬛⬛⬛　⬛⬛⬛　⬛⬛".truecolor(200, 171, 55));
    println!("⬜{}⬜", "　　⬛　　　　　　　　　　　　　　　　　　　　　　　　　".truecolor(200, 171, 55));
    println!("⬜{}⬜", "⬛⬛⬛　　　　　　　　　　　　　　　　　　　　　　　　　".truecolor(200, 171, 55));
    println!("⬜{}⬜", "⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜⬜");
}

pub fn build(record: bool, debug: bool) -> Result<(), Box<dyn Error>> {
    print_banner();

    let start_time = Instant::now();
    let context = BuildContext::new(Config::load()?, record, debug);

    // Prepare to build templates
    let profiles = goldboot_templates::get_templates(&context.config)?;
    let profiles_len = profiles.len();
    if profiles_len == 0 {
        bail!("At least one base profile must be specified");
    }

    // Create an initial image that will be attached as storage to each VM
    debug!(
        "Allocating new {} image: {}",
        context.config.disk_size, context.image_path
    );
    goldboot_image::Qcow2::create(
        &context.image_path,
        context.config.disk_size_bytes(),
        serde_json::to_vec(&context.config)?,
    )?;

    // Create partitions if we're multi booting
    if profiles.len() > 1 {
        // TODO
    }

    // Build each profile
    for profile in profiles {
        profile.build(&context)?;
    }

    // Install bootloader if we're multi booting
    if profiles_len > 1 {
        // TODO
    }

    // Attempt to reduce the size of image
    compact_image(&context.image_path)?;

    info!("Build completed in: {:?}", start_time.elapsed());

    // Create new image metadata
    // TODO
    let metadata = ImageMetadata {
        sha256: String::from(""),
        size: 0,
        last_modified: 0,
        config: context.config,
    };

    // Move the image to the library
    std::fs::copy(context.image_path, metadata.path_qcow2())?;

    Ok(())
}
