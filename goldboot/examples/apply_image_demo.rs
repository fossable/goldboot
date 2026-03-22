//! Demo of the apply_image screen with simulated image writing.
//!
//! Run with: cargo run --example apply_image_demo --features gui

use goldboot::gui::{
    app::GuiApp,
    screens::Screen,
    state::{AppState, WriteProgress},
};
use std::{
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

fn main() -> Result<(), eframe::Error> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    let mut state = AppState::new();

    // Simulate a 1 GB image with 4 MB blocks
    let block_size: u64 = 4 * 1024 * 1024;
    let cluster_count: usize = 256; // 256 × 4 MB = 1 GB
    let progress = Arc::new(Mutex::new(WriteProgress::new(cluster_count, block_size)));
    state.write_progress = Some(progress.clone());

    // Background thread: simulate writing one cluster every ~10 ms
    thread::spawn(move || {
        for idx in 0..cluster_count {
            thread::sleep(Duration::from_millis(10));
            // Simulate ~30% of blocks already being up to date
            let is_dirty = (idx % 10) >= 3;
            if is_dirty {
                if let Ok(mut p) = progress.lock() {
                    p.record_cluster(idx, None); // Writing
                }
                thread::sleep(Duration::from_millis(5)); // simulate write time
            }
            if let Ok(mut p) = progress.lock() {
                p.record_cluster(idx, Some(is_dirty));
            }
        }
        if let Ok(mut p) = progress.lock() {
            p.done = true;
        }
    });

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1024.0, 768.0])
            .with_title("Goldboot - Apply Image Demo"),
        ..Default::default()
    };

    eframe::run_native(
        "goldboot",
        native_options,
        Box::new(move |cc| {
            Ok(Box::new(GuiApp {
                state,
                textures: goldboot::gui::resources::TextureCache::new(&cc.egui_ctx),
                theme: goldboot::gui::theme::Theme::default(),
                screen: Screen::ApplyImage,
            }))
        }),
    )
}
