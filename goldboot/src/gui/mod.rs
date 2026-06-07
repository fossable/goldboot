pub mod app;
pub mod resources;
pub mod screens;
pub mod state;
pub mod theme;
pub mod widgets;

use std::process::ExitCode;

use tracing::{debug, error};

pub fn run_gui(fullscreen: bool) -> ExitCode {
    // In UKI mode, we always run as root (booted directly from firmware)
    #[cfg(feature = "uki")]
    let try_sudo = false;

    #[cfg(not(feature = "uki"))]
    let try_sudo = !whoami::username().map(|u| u == "root").unwrap_or(false);

    let mut viewport = egui::ViewportBuilder::default()
        .with_fullscreen(fullscreen)
        .with_decorations(!fullscreen)
        .with_title("goldboot");

    if !fullscreen {
        viewport = viewport.with_inner_size([1920.0, 1080.0]);
    }

    debug!("Starting GUI");

    let native_options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    match eframe::run_native(
        "goldboot",
        native_options,
        Box::new(move |cc| Ok(Box::new(app::GuiApp::new(cc, try_sudo)))),
    ) {
        Ok(_) => ExitCode::SUCCESS,
        Err(e) => {
            error!(error = ?e, "Failed to start GUI");
            ExitCode::FAILURE
        }
    }
}
