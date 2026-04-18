use super::super::{
    resources::TextureCache,
    state::{AppState, BlockState, PostWriteDialog},
    theme::Theme,
    widgets,
};
use super::Screen;

#[derive(Debug, Clone, Copy, PartialEq)]
enum DialogChoice {
    Verify,
    Reboot,
    Exit,
}

impl DialogChoice {
    fn next(self) -> Self {
        match self {
            Self::Verify => Self::Reboot,
            Self::Reboot => Self::Exit,
            Self::Exit => Self::Exit,
        }
    }
    fn prev(self) -> Self {
        match self {
            Self::Verify => Self::Verify,
            Self::Reboot => Self::Verify,
            Self::Exit => Self::Reboot,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Verify => "Verify written data",
            Self::Reboot => "Reboot",
            Self::Exit => "Exit",
        }
    }
    fn hotkey(self) -> &'static str {
        match self {
            Self::Verify => "V",
            Self::Reboot => "R",
            Self::Exit => "E",
        }
    }
}

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
    // Error dialog overlay
    if widgets::error_dialog::render(ui, &mut state.error_message, theme) {
        return;
    }

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

    // Post-write/verify dialog (overlays everything)
    let (show_dialog, verified) = state
        .write_progress
        .as_ref()
        .and_then(|p| p.lock().ok())
        .map(|p| {
            let show = p.post_write_dialog == Some(PostWriteDialog::Visible);
            // Verified means either verify_only mode, or blocks have been verified
            let verified = p.verify_only || p.blocks_verified() > 0;
            (show, verified)
        })
        .unwrap_or((false, false));

    if show_dialog {
        // Dialog selection state lives in egui memory keyed by a fixed id
        let dialog_id = egui::Id::new("post_write_dialog_choice");
        let default_choice = if verified {
            DialogChoice::Reboot
        } else {
            DialogChoice::Verify
        };
        let mut choice = ui
            .ctx()
            .memory(|m| m.data.get_temp::<DialogChoice>(dialog_id))
            .unwrap_or(default_choice);

        // If verified, don't allow Verify choice
        if verified && choice == DialogChoice::Verify {
            choice = DialogChoice::Reboot;
        }

        let mut action: Option<DialogChoice> = None;

        ui.ctx().input(|inp| {
            if inp.key_pressed(egui::Key::ArrowDown) {
                choice = choice.next();
                if verified && choice == DialogChoice::Verify {
                    choice = choice.next();
                }
            }
            if inp.key_pressed(egui::Key::ArrowUp) {
                choice = choice.prev();
                if verified && choice == DialogChoice::Verify {
                    // Verify is first, so stay at Reboot
                    choice = DialogChoice::Reboot;
                }
            }
            if inp.key_pressed(egui::Key::Enter) {
                action = Some(choice);
            }
            // Direct hotkeys
            let available_opts: Vec<DialogChoice> = if verified {
                vec![DialogChoice::Reboot, DialogChoice::Exit]
            } else {
                vec![
                    DialogChoice::Verify,
                    DialogChoice::Reboot,
                    DialogChoice::Exit,
                ]
            };
            for opt in available_opts {
                if inp
                    .events
                    .iter()
                    .any(|e| matches!(e, egui::Event::Text(t) if t.to_uppercase() == opt.hotkey()))
                {
                    action = Some(opt);
                }
            }
        });

        ui.ctx()
            .memory_mut(|m| m.data.insert_temp(dialog_id, choice));

        if let Some(act) = action {
            match act {
                DialogChoice::Verify => {
                    if let Some(p) = &state.write_progress {
                        if let Ok(mut p) = p.lock() {
                            p.post_write_dialog = Some(PostWriteDialog::Hidden);
                        }
                    }
                    if let Err(e) = state.start_verify() {
                        tracing::error!(error = %e, "Failed to start verification");
                        state.error_message = Some(e);
                    }
                }
                DialogChoice::Reboot => {
                    let _ = std::process::Command::new("reboot").status();
                }
                DialogChoice::Exit => {
                    std::process::exit(0);
                }
            }
        }

        // Hotkeys footer
        egui::TopBottomPanel::bottom("post_write_hotkeys")
            .frame(egui::Frame::NONE)
            .show_separator_line(false)
            .show_inside(ui, |ui| {
                let mut hotkeys = vec![("↑↓", "Navigate"), ("Enter", "Select")];
                if !verified {
                    hotkeys.push(("V", "Verify"));
                }
                hotkeys.push(("R", "Reboot"));
                hotkeys.push(("E", "Exit"));
                widgets::hotkeys::render(ui, &hotkeys, theme);
            });

        ui.centered_and_justified(|ui| {
            let box_width = 420.0;
            let box_height = 220.0;
            let box_rect = egui::Rect::from_center_size(
                ui.available_rect_before_wrap().center(),
                egui::vec2(box_width, box_height),
            );

            let painter = ui.painter();
            painter.rect_filled(
                box_rect,
                8.0,
                egui::Color32::from_rgba_unmultiplied(0x22, 0x22, 0x22, 245),
            );
            painter.rect_stroke(
                box_rect,
                8.0,
                egui::Stroke::new(2.0, theme.accent_gold),
                egui::StrokeKind::Outside,
            );

            ui.allocate_new_ui(
                egui::UiBuilder::new().max_rect(box_rect.shrink(24.0)),
                |ui| {
                    ui.vertical_centered(|ui| {
                        ui.label(
                            egui::RichText::new(if verified {
                                "Verify Complete"
                            } else {
                                "Write Complete"
                            })
                            .color(theme.accent_gold)
                            .strong()
                            .size(20.0),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new("What would you like to do next?")
                                .color(theme.text_secondary)
                                .size(13.0),
                        );
                        ui.add_space(20.0);

                        let options: Vec<DialogChoice> = if verified {
                            vec![DialogChoice::Reboot, DialogChoice::Exit]
                        } else {
                            vec![
                                DialogChoice::Verify,
                                DialogChoice::Reboot,
                                DialogChoice::Exit,
                            ]
                        };
                        for opt in options {
                            let is_selected = choice == opt;
                            let text =
                                egui::RichText::new(format!("[{}] {}", opt.hotkey(), opt.label()))
                                    .size(14.0)
                                    .color(if is_selected {
                                        theme.accent_gold
                                    } else {
                                        theme.text_primary
                                    });
                            ui.label(text);
                            ui.add_space(6.0);
                        }
                    });
                },
            );
        });
        return;
    }

    // Progress box
    ui.centered_and_justified(|ui| {
        if let Some(progress_arc) = &state.write_progress {
            if let Ok(progress) = progress_arc.lock() {
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
                    egui::StrokeKind::Outside,
                );

                ui.allocate_new_ui(
                    egui::UiBuilder::new().max_rect(box_rect.shrink(20.0)),
                    |ui| {
                        ui.vertical_centered(|ui| {
                            ui.label(
                                egui::RichText::new(if progress.verifying {
                                    "Verifying Image"
                                } else {
                                    "Writing Image"
                                })
                                .color(theme.accent_gold)
                                .strong()
                                .size(20.0),
                            );

                            ui.add_space(15.0);

                            let pct = progress.percentage();
                            ui.label(
                                egui::RichText::new(format!("{:.1}%", pct * 100.0))
                                    .color(theme.text_primary)
                                    .size(32.0)
                                    .strong(),
                            );

                            ui.add_space(10.0);

                            let progress_bar = egui::ProgressBar::new(pct)
                                .fill(theme.accent_gold)
                                .animate(true);
                            ui.add_sized([box_width - 40.0, 20.0], progress_bar);

                            ui.add_space(15.0);

                            if progress.verifying {
                                // Verify-only: show just read speed
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
                            } else {
                                // Writing: show both read and write speeds
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
                            }

                            ui.add_space(10.0);

                            if progress.verifying {
                                let failed = progress.blocks_failed();
                                let verified = progress.blocks_verified();
                                let label = if failed > 0 {
                                    format!(
                                        "Verified: {}  Failed: {}  /  {}",
                                        verified, failed, progress.cluster_count
                                    )
                                } else {
                                    format!("Verified: {}/{}", verified, progress.cluster_count)
                                };
                                ui.label(
                                    egui::RichText::new(label)
                                        .color(if failed > 0 {
                                            egui::Color32::from_rgb(0xff, 0x60, 0x60)
                                        } else {
                                            theme.text_secondary
                                        })
                                        .size(12.0),
                                );
                            } else {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "Blocks: {}/{} written, {} up to date",
                                            progress.blocks_written(),
                                            progress.cluster_count,
                                            progress.blocks_up_to_date(),
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
                            }

                            if let Some(err) = &progress.error {
                                ui.add_space(8.0);
                                ui.label(
                                    egui::RichText::new(format!("Error: {err}"))
                                        .color(egui::Color32::from_rgb(0xff, 0x60, 0x60))
                                        .size(12.0),
                                );
                            }
                        });
                    },
                );
            }
        }
    });

    // Keep repainting while work is in progress
    if state
        .write_progress
        .as_ref()
        .and_then(|p| p.lock().ok())
        .map(|p| !p.done)
        .unwrap_or(false)
    {
        ui.ctx().request_repaint();
    }
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

    let cols = (rect.width() / BLOCK_TOTAL).floor() as usize;
    let rows = (rect.height() / BLOCK_TOTAL).floor() as usize;
    let total_grid_cells = cols * rows;

    let grid_width = cols as f32 * BLOCK_TOTAL;
    let grid_height = rows as f32 * BLOCK_TOTAL;
    let start_x = rect.min.x + (rect.width() - grid_width) / 2.0;
    let start_y = rect.min.y + (rect.height() - grid_height) / 2.0;

    let cluster_count = progress.cluster_count;

    for row in 0..rows {
        for col in 0..cols {
            let grid_idx = row * cols + col;

            let state = if cluster_count >= total_grid_cells {
                let start = grid_idx * cluster_count / total_grid_cells;
                let end = ((grid_idx + 1) * cluster_count / total_grid_cells).min(cluster_count);
                aggregate_states(&progress.block_states[start..end])
            } else {
                let idx = grid_idx * cluster_count / total_grid_cells;
                if idx >= cluster_count {
                    continue;
                }
                progress.block_states[idx]
            };

            let color = match state {
                BlockState::Pending => egui::Color32::from_rgba_unmultiplied(0x4a, 0x4a, 0x4a, 100),
                BlockState::Writing | BlockState::Verifying => theme.accent_gold,
                BlockState::UpToDate => egui::Color32::from_rgb(0x3a, 0x5a, 0x8a),
                BlockState::Written | BlockState::Verified => {
                    egui::Color32::from_rgb(0x5a, 0x8a, 0x5a)
                }
                BlockState::Failed => egui::Color32::from_rgb(0xaa, 0x33, 0x33),
            };

            let x = start_x + col as f32 * BLOCK_TOTAL;
            let y = start_y + row as f32 * BLOCK_TOTAL;
            let block_rect =
                egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(BLOCK_SIZE, BLOCK_SIZE));
            painter.rect_filled(block_rect, 1.0, color);
        }
    }
}

/// Pick the most "interesting" state from a slice of block states for grid display.
fn aggregate_states(states: &[BlockState]) -> BlockState {
    if states.iter().any(|&s| s == BlockState::Failed) {
        BlockState::Failed
    } else if states
        .iter()
        .any(|&s| s == BlockState::Writing || s == BlockState::Verifying)
    {
        states
            .iter()
            .copied()
            .find(|&s| s == BlockState::Writing || s == BlockState::Verifying)
            .unwrap()
    } else if states.iter().any(|&s| s == BlockState::Written) {
        BlockState::Written
    } else if states.iter().any(|&s| s == BlockState::Verified) {
        BlockState::Verified
    } else if states.iter().any(|&s| s == BlockState::Pending) {
        BlockState::Pending
    } else {
        BlockState::UpToDate
    }
}
