//! Demo of the apply_image screen with simulated image writing.
//!
//! This example demonstrates the block visualization and progress tracking in the GUI
//! by simulating an image write operation with realistic speeds and progress updates.
//!
//! Run with: cargo run --example apply_image_demo --features gui

use goldboot::gui::{
    app::GuiApp,
    state::{AppState, BlockState, WriteProgress},
    screens::Screen,
};
use std::{
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

fn main() -> Result<(), eframe::Error> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    // Create a custom app state that starts directly on the ApplyImage screen
    let mut state = AppState::new();

    // Simulate a 1GB image write
    let total_size = 1024 * 1024 * 1024u64;
    state.init_write_progress(total_size);

    // Clone the progress Arc for the write thread
    let progress_arc = state.write_progress.clone().unwrap();

    // Spawn a background thread to perform simulated write with realistic speeds
    thread::spawn(move || {
        let start_time = Instant::now();
        let mut last_update = Instant::now();

        // Simulate write speed of ~400 MB/s
        let target_write_speed = 400_000_000.0; // bytes per second
        let mut bytes_written = 0u64;
        let block_size = 4 * 1024 * 1024u64; // 4MB blocks

        while bytes_written < total_size {
            // Sleep to simulate realistic write speed
            thread::sleep(Duration::from_millis(10));

            let now = Instant::now();
            let elapsed_ms = now.duration_since(last_update).as_millis() as f64;

            // Calculate how many bytes we should have written in this interval
            let bytes_to_write = ((target_write_speed / 1000.0) * elapsed_ms) as u64;
            let bytes_to_write = bytes_to_write.min(total_size - bytes_written);

            bytes_written += bytes_to_write;
            last_update = now;

            if let Ok(mut progress) = progress_arc.lock() {
                // Update percentage
                progress.percentage = bytes_written as f32 / total_size as f32;

                // Calculate current block index
                let current_block = (bytes_written / block_size) as usize;
                let current_block = current_block.min(progress.blocks_total.saturating_sub(1));

                // Mark completed blocks as written
                for i in 0..current_block {
                    if i < progress.block_states.len() && progress.block_states[i] != BlockState::Written {
                        progress.block_states[i] = BlockState::Written;
                        progress.blocks_written += 1;
                    }
                }

                // Mark current block(s) as writing (simulate 2-3 blocks writing simultaneously)
                progress.blocks_writing = 0;
                for i in current_block..=(current_block + 2).min(progress.blocks_total.saturating_sub(1)) {
                    if i < progress.block_states.len() && progress.block_states[i] == BlockState::Pending {
                        progress.block_states[i] = BlockState::Writing;
                        progress.blocks_writing += 1;
                    }
                }

                // Update speeds with some realistic variation
                let speed_variation = (rand::random::<f64>() - 0.5) * 0.1; // ±10% variation
                progress.write_speed = target_write_speed * (1.0 + speed_variation);
                progress.read_speed = progress.write_speed * 1.05; // Read slightly faster
            }
        }

        // Mark all blocks as written when complete
        if let Ok(mut progress) = progress_arc.lock() {
            progress.percentage = 1.0;
            for i in 0..progress.blocks_total {
                if i < progress.block_states.len() && progress.block_states[i] != BlockState::Written {
                    progress.block_states[i] = BlockState::Written;
                    progress.blocks_written += 1;
                }
            }
            progress.blocks_writing = 0;
        }

        println!("Simulated write completed in {:.2}s", start_time.elapsed().as_secs_f64());
    });

    // Create and run the GUI app starting on ApplyImage screen
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1024.0, 768.0])
            .with_title("Goldboot - Apply Image Demo"),
        ..Default::default()
    };

    // Create app with custom initial state
    eframe::run_native(
        "goldboot",
        native_options,
        Box::new(move |cc| {
            Ok(Box::new(GuiApp {
                state,
                textures: goldboot::gui::resources::TextureCache::new(&cc.egui_ctx),
                theme: goldboot::gui::theme::Theme::default(),
                screen: Screen::ApplyImage, // Start directly on ApplyImage
            }))
        }),
    )
}
