use std::{
	path::PathBuf,
	process::Command,
    fs,
    error::Error,
};
use crate::{
	config::Config,
	packer::{PackerTemplate, PackerProvisioner},
    profiles,
    image_library_path,
};
use log::{debug};
use anyhow::bail;

pub struct BuildContext {
	pub config: Config,
	pub template: PackerTemplate,

	/// The temporary build directory which will be deleted at the end of the run
	pub directory: PathBuf,
}

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

    if let Some(profile) = &config.base {

    	// Create packer template
        let mut template = PackerTemplate::default();

        // Run profile-specific build hook
        match profile.as_str() {
            "ArchLinux" => profiles::arch_linux::build(&config, &context_path),
            "Windows10" => profiles::windows_10::build(&config, &context_path),
            "PopOs2104" => profiles::pop_os_2104::build(&config, &context_path),
            "PopOs2110" => profiles::pop_os_2110::build(&config, &context_path),
        }?;

        // Builder overrides
        builder.output_directory = image_library_path().join("output").to_str().unwrap().to_string();
        builder.vm_name = Some(config.name.to_string());
        builder.qemuargs = Some(config.qemu.to_qemuargs());
        builder.memory = config.memory.to_string();
        builder.disk_size = config.disk_size.to_string();
        if let Some(arch) = config.arch {
            builder.qemu_binary = match arch {
                "x86_64" => Some("qemu-system-x86_64".into()),
                _ => None,
            };
        }

        if config.iso_url != "" {
            builder.iso_url = config.iso_url.to_string();
        }

        if let Some(checksum) = config.iso_checksum {
            builder.iso_checksum = checksum.to_string();
        } else {
            builder.iso_checksum = "none".into();
        }

        template.builders.push(builder);

        // Translate provisioners in config into packer provisioners
        for p in config.provisioners.iter() {
            let provisioner = match p.r#type.as_str() {
                "ansible" => PackerProvisioner {
                    r#type: "ansible".into(),
                    scripts: None,
                    playbook_file: Some(p.ansible.playbook.as_ref().unwrap().clone()),
                    user: Some("root".into()),
                    use_proxy: Some(false),
                    extra_arguments: vec![
                        "-e",
                        "ansible_winrm_scheme=http",
                        "-e",
                        "ansible_winrm_server_cert_validation=ignore",
                        "-e",
                        "ansible_ssh_pass=root",
                    ],
                },
                _ => panic!(""),
            };
            template.provisioners.push(provisioner);
        }

        debug!("Created packer template: {:?}", &template);

        // Write the packer template
        fs::write(
            context_path.join("packer.json"),
            serde_json::to_string(&template).unwrap(),
        )
        .unwrap();
    }

    // Run the build
    if let Some(code) = Command::new("packer")
        .current_dir(context_path)
        .arg("build")
        .arg("packer.json")
        .status()
        .expect("Failed to launch packer")
        .code()
    {
        if code != 0 {
            bail!("Build failed with error code: {}", code);
        }
    } else {
        bail!("");
    }

    debug!("Build completed successfully");

    // Create new image metadata
    let metadata_name = ImageMetadata::new(image_library_path().join("output").join(&config.name))?.write()?;

    // Move the image to the library
    fs::rename(
        image_library_path().join("output").join(&config.name),
        image_library_path().join(format!("{}.qcow2", &metadata_name)),
    )
    .unwrap();

    return Ok(());
}