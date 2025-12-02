use super::super::{resources::TextureCache, state::{AppState, BlockState}, theme::Theme, widgets};
use super::Screen;

fn format_speed(bytes_per_sec: f64) -> String {
    if bytes_per_sec >= 1_000_000_000.0 {
        format!("{:.2} GB/s", bytes_per_sec / 1_000_000_000.0)
    } else if bytes_per_sec >= 1_000_000.0 {
        format!("{:.2} MB/s", bytes_per_sec / 1_000_000.0)
    } else if bytes_per_sec >= 1_000.0 {
        format!("{:.2} KB/s", bytes_per_sec / 1_000.0)
    } else {
        format!("{:.0} B/s", bytes_per_sec)
    }
}

fn format_time(seconds: f64) -> String {
    let hours = (seconds / 3600.0) as u32;
    let minutes = ((seconds % 3600.0) / 60.0) as u32;
    let secs = (seconds % 60.0) as u32;

    if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, secs)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, secs)
    } else {
        format!("{}s", secs)
    }
}

pub fn render(
    ui: &mut egui::Ui,
    state: &mut AppState,
    _textures: &TextureCache,
    theme: &Theme,
    _screen: &mut Screen,
) {
    // Get write progress (or create demo progress)
    let write_progress = state.write_progress.clone();

    if let Some(progress_arc) = write_progress {
        // Simulate progress for demo
        if let Ok(mut progress) = progress_arc.lock() {
            // Simulate writing blocks
            if progress.percentage < 1.0 {
                progress.percentage += 0.001; // Slow increment for demo

                // Simulate block states
                let current_block = (progress.percentage * progress.blocks_total as f32) as usize;
                let current_block = current_block.min(progress.blocks_total - 1);

                // Mark some blocks as written
                for i in 0..current_block {
                    if progress.block_states[i] != BlockState::Written {
                        progress.block_states[i] = BlockState::Written;
                        progress.blocks_written += 1;
                    }
                }

                // Mark current blocks as writing (2-3 blocks at a time)
                progress.blocks_writing = 0;
                for i in current_block..=(current_block + 2).min(progress.blocks_total - 1) {
                    if progress.block_states[i] == BlockState::Pending {
                        progress.block_states[i] = BlockState::Writing;
                        progress.blocks_writing += 1;
                    }
                }

                // Simulate speeds
                progress.read_speed = 450_000_000.0 + (rand::random::<f64>() - 0.5) * 50_000_000.0;
                progress.write_speed = 420_000_000.0 + (rand::random::<f64>() - 0.5) * 40_000_000.0;
            }
        }
    }

    // Render the screen
    let available_rect = ui.available_rect_before_wrap();

    // Draw block grid as background
    {
        let painter = ui.painter();
        if let Some(progress_arc) = &state.write_progress {
            if let Ok(progress) = progress_arc.lock() {
                render_block_grid(painter, &available_rect, &progress, theme);
            }
        }
    }

    // Center progress box (painter borrow dropped)
    ui.centered_and_justified(|ui| {
        if let Some(progress_arc) = &state.write_progress {
            if let Ok(progress) = progress_arc.lock() {
                // Semi-transparent dark background box
                let box_width = 500.0;
                let box_height = 250.0;

                let box_rect = egui::Rect::from_center_size(
                    ui.available_rect_before_wrap().center(),
                    egui::vec2(box_width, box_height),
                );

                let painter = ui.painter();
                painter.rect_filled(
                    box_rect,
                    5.0,
                    egui::Color32::from_rgba_unmultiplied(0x33, 0x33, 0x33, 230),
                );

                painter.rect_stroke(
                    box_rect,
                    5.0,
                    egui::Stroke::new(3.0, theme.accent_gold),
                );

                // Render content inside the box
                ui.allocate_ui_at_rect(box_rect.shrink(20.0), |ui| {
                    ui.vertical_centered(|ui| {
                        // Title
                        ui.label(
                            egui::RichText::new("Writing Image to Device")
                                .color(theme.accent_gold)
                                .strong()
                                .size(20.0),
                        );

                        ui.add_space(15.0);

                        // Progress percentage
                        ui.label(
                            egui::RichText::new(format!("{:.1}%", progress.percentage * 100.0))
                                .color(theme.text_primary)
                                .size(32.0)
                                .strong(),
                        );

                        ui.add_space(10.0);

                        // Progress bar
                        let progress_bar = egui::ProgressBar::new(progress.percentage)
                            .fill(theme.accent_gold)
                            .animate(true);
                        ui.add_sized([box_width - 40.0, 20.0], progress_bar);

                        ui.add_space(15.0);

                        // Stats in two columns
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(
                                    egui::RichText::new("Read Speed:")
                                        .color(theme.text_secondary)
                                        .size(14.0),
                                );
                                ui.label(
                                    egui::RichText::new(format_speed(progress.read_speed))
                                        .color(theme.text_primary)
                                        .size(16.0)
                                        .strong(),
                                );
                            });

                            ui.add_space(40.0);

                            ui.vertical(|ui| {
                                ui.label(
                                    egui::RichText::new("Write Speed:")
                                        .color(theme.text_secondary)
                                        .size(14.0),
                                );
                                ui.label(
                                    egui::RichText::new(format_speed(progress.write_speed))
                                        .color(theme.text_primary)
                                        .size(16.0)
                                        .strong(),
                                );
                            });
                        });

                        ui.add_space(10.0);

                        // Additional info
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(format!(
                                    "Blocks: {}/{} ({})",
                                    progress.blocks_written,
                                    progress.blocks_total,
                                    progress.blocks_writing
                                ))
                                .color(theme.text_secondary)
                                .size(12.0),
                            );

                            ui.add_space(20.0);

                            ui.label(
                                egui::RichText::new(format!(
                                    "Elapsed: {}",
                                    format_time(progress.elapsed_seconds())
                                ))
                                .color(theme.text_secondary)
                                .size(12.0),
                            );
                        });
                    });
                });
            }
        }
    });

    // Request repaint for animation
    ui.ctx().request_repaint();
}

fn render_block_grid(
    painter: &egui::Painter,
    rect: &egui::Rect,
    progress: &super::super::state::WriteProgress,
    theme: &Theme,
) {
    const BLOCK_SIZE: f32 = 10.0;
    const BLOCK_SPACING: f32 = 2.0;
    const BLOCK_TOTAL: f32 = BLOCK_SIZE + BLOCK_SPACING;

    // Calculate grid dimensions
    let cols = (rect.width() / BLOCK_TOTAL).floor() as usize;
    let rows = (rect.height() / BLOCK_TOTAL).floor() as usize;
    let total_grid_blocks = cols * rows;

    // Calculate starting position to center the grid
    let grid_width = cols as f32 * BLOCK_TOTAL;
    let grid_height = rows as f32 * BLOCK_TOTAL;
    let start_x = rect.min.x + (rect.width() - grid_width) / 2.0;
    let start_y = rect.min.y + (rect.height() - grid_height) / 2.0;

    // Map progress blocks to grid blocks
    let blocks_per_grid = if progress.blocks_total > 0 {
        total_grid_blocks as f32 / progress.blocks_total as f32
    } else {
        1.0
    };

    // Draw blocks
    for row in 0..rows {
        for col in 0..cols {
            let grid_idx = row * cols + col;
            let progress_idx = (grid_idx as f32 / blocks_per_grid) as usize;

            if progress_idx >= progress.blocks_total {
                continue;
            }

            let block_state = progress.block_states[progress_idx];

            let color = match block_state {
                BlockState::Pending => egui::Color32::from_rgba_unmultiplied(0x4a, 0x4a, 0x4a, 100),
                BlockState::Writing => theme.accent_gold,
                BlockState::Written => egui::Color32::from_rgb(0x5a, 0x8a, 0x5a), // Green
            };

            let x = start_x + col as f32 * BLOCK_TOTAL;
            let y = start_y + row as f32 * BLOCK_TOTAL;

            let block_rect = egui::Rect::from_min_size(
                egui::pos2(x, y),
                egui::vec2(BLOCK_SIZE, BLOCK_SIZE),
            );

            painter.rect_filled(block_rect, 1.0, color);
        }
    }
}
