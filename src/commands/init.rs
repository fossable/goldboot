use crate::{config::Config, profiles, qemu::QemuConfig};
use std::{env, error::Error, fs, path::Path};
use simple_error::bail;

/// Choose some arbitrary disk and get its size. The user will likely change it
/// in the config later.
fn guess_disk_size() -> u64 {
    if cfg!(target_os = "unix") {
        // TODO
    }
    return 256000000;
}

pub fn init(profile: Option<String>, template: Option<String>) -> Result<(), Box<dyn Error>> {
    let config_path = Path::new("goldboot.json");

    if config_path.exists() {
        bail!("This directory has already been initialized. Delete goldboot.json to reinitialize.");
    }

    // Create a new config to be filled in according to the given arguments
    let mut config = Config::default();

    // Setup the config for the given base profile
    if let Some(profile_value) = profile {
        // Set name equal to directory name
        if let Some(name) = env::current_dir()?.file_name() {
            config.name = name.to_str().unwrap().to_string();
        }

        // Generate QEMU flags for this hardware
        config.qemu = QemuConfig::generate_config()?;

        // Set current platform
        config.arch = if cfg!(target_arch = "x86_64") {
            Some("x86_64".into())
        } else if cfg!(target_arch = "aarch64") {
            Some("aarch64".into())
        } else {
            panic!("Unsupported platform");
        };

        // Set an arbitrary disk size
        config.disk_size = format!("{}b", guess_disk_size());

        // Run profile-specific initialization
        match profile_value.as_str() {
            "ArchLinux" => profiles::arch_linux::init(&mut config),
            "Windows10" => profiles::windows_10::init(&mut config),
            "UbuntuServer2110" => profiles::ubuntu_server_2110::init(&mut config),
            "PopOs2010" => profiles::pop_os_2110::init(&mut config),
            _ => panic!("Unknown profile"),
        }
    }

    // Setup the config for the given packer template
    if let Some(template_value) = template {
        config.packer_template = Some(template_value);
    }

    // Finally write out the config
    fs::write(config_path, serde_json::to_string_pretty(&config)?)?;
    Ok(())
}
