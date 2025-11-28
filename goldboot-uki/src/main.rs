use std::process::ExitCode;
use tracing::info;

pub mod gui;

fn main() -> ExitCode {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    info!("Starting goldboot-uki");

    // In UKI mode, always run fullscreen
    let fullscreen = true;

    // Ensure we're running in the correct environment
    if let Err(e) = check_environment() {
        eprintln!("Environment check failed: {}", e);
        return ExitCode::FAILURE;
    }

    // Run the GUI
    let result = crate::gui::load_gui(fullscreen);

    // After GUI exits, perform system shutdown
    info!("GUI exited, initiating system reboot");
    if let Err(e) = reboot_system() {
        eprintln!("Failed to reboot system: {}", e);
        return ExitCode::FAILURE;
    }

    result
}

/// Check that we're running in the expected initramfs environment
fn check_environment() -> Result<(), String> {
    // Verify we have access to block devices
    if !std::path::Path::new("/sys/class/block").exists() {
        return Err("Block device sysfs not available".to_string());
    }

    // Verify the image library directory exists
    let lib_path = std::path::Path::new("/var/lib/goldboot/images");
    if !lib_path.exists() {
        std::fs::create_dir_all(lib_path)
            .map_err(|e| format!("Failed to create library directory: {}", e))?;
    }

    Ok(())
}

/// Reboot the system (called when GUI exits)
fn reboot_system() -> Result<(), String> {
    use std::process::Command;

    // Try systemctl first (if systemd is available)
    let result = Command::new("systemctl")
        .arg("reboot")
        .status();

    if result.is_ok() {
        return Ok(());
    }

    // Fallback to direct reboot command
    Command::new("reboot")
        .status()
        .map_err(|e| format!("Failed to execute reboot: {}", e))?;

    Ok(())
}
