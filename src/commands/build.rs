use crate::commands::image::ImageMetadata;
use crate::{config::Config, image_library_path, packer::PackerTemplate, profile::Profile};
use log::debug;
use simple_error::bail;
use std::{error::Error, fs, process::Command};

pub fn build() -> Result<(), Box<dyn Error>> {
    debug!("Starting build");

    // Load config
    let config = Config::load()?;

    // Acquire temporary directory for the build
    let tmp = tempfile::tempdir().unwrap();
    let context_path = tmp.path();
    debug!(
        "Allocated temporary directory for build: {}",
        context_path.display()
    );

    let mut templates = Vec::<PackerTemplate>::new();

    if let Some(profile) = &config.ArchLinux {
        templates.push(profile.generate_template(context_path)?);
    }
    if let Some(profile) = &config.Windows10 {
        templates.push(profile.generate_template(context_path)?);
    }
    if let Some(profile) = &config.PopOs {
        templates.push(profile.generate_template(context_path)?);
    }
    if let Some(profile) = &config.SteamOs {
        templates.push(profile.generate_template(context_path)?);
    }
    if let Some(profile) = &config.SteamDeck {
        templates.push(profile.generate_template(context_path)?);
    }

    // Configure the builder for each template
    for template in templates.iter_mut() {
        let builder = template.builders.first_mut().unwrap();
        builder.output_directory = image_library_path()
            .join("output")
            .to_str()
            .unwrap()
            .to_string();
        builder.vm_name = Some(config.name.to_string());
        //builder.qemuargs = Some(config.qemu.to_qemuargs());
        builder.memory = config.memory.to_string();
        builder.disk_size = config.disk_size.to_string();
        if let Some(arch) = &config.arch {
            builder.qemu_binary = match arch.as_str() {
                "x86_64" => Some("qemu-system-x86_64".into()),
                _ => None,
            };
        }
    }

    // Execute the templates sequentially
    for template in templates {
        // Write the template to the context directory
        fs::write(
            context_path.join("packer.json"),
            serde_json::to_string(&template).unwrap(),
        )
        .unwrap();

        // Run the build
        if let Some(code) = Command::new("packer")
            .current_dir(context_path)
            .arg("build")
            .arg("-force")
            .arg("packer.json")
            .status()
            .expect("Failed to launch packer")
            .code()
        {
            if code != 0 {
                bail!("Build failed with error code: {}", code);
            }
        } else {
            panic!();
        }
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
