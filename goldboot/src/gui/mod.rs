pub mod app;
pub mod resources;
pub mod screens;
pub mod state;
pub mod theme;
pub mod widgets;

use std::process::ExitCode;

pub fn run_gui(fullscreen: bool) -> ExitCode {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1920.0, 1080.0])
            .with_fullscreen(fullscreen)
            .with_decorations(!fullscreen)
            .with_title("goldboot"),
        ..Default::default()
    };

    match eframe::run_native(
        "goldboot",
        native_options,
        Box::new(|cc| Ok(Box::new(app::GuiApp::new(cc)))),
    ) {
        Ok(_) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("GUI error: {}", e);
            ExitCode::FAILURE
        }
    }
}
