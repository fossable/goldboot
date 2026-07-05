//! Chain-loader menu shown when goldboot.efi detected other bootloaders.
//!
//! The selected target is chain-loaded by registering a boot entry with
//! `BootNext` and rebooting; firmware then loads the real bootloader. A
//! countdown auto-boots the selection unless a key is pressed, and F10
//! leads to the normal image deployment flow.

use super::super::{resources::TextureCache, state::AppState, theme::Theme, widgets};
use super::Screen;

/// Map a loader's vendor directory to the CamelCase os_name the texture
/// cache indexes icons by.
fn icon_name(loader_dir: &str) -> Option<&'static str> {
    Some(match loader_dir.to_ascii_lowercase().as_str() {
        "alpine" => "AlpineLinux",
        "arch" => "ArchLinux",
        "debian" => "Debian",
        "ubuntu" => "Ubuntu",
        "fedora" => "Fedora",
        "nixos" => "Nix",
        _ => return None,
    })
}

pub fn render(
    ui: &mut egui::Ui,
    state: &mut AppState,
    textures: &TextureCache,
    theme: &Theme,
    screen: &mut Screen,
) {
    // Error dialog overlay (e.g. a failed chain-load)
    if widgets::error_dialog::render(ui, &mut state.error_message, theme) {
        return;
    }

    let mut boot_now = false;

    // Countdown: any keypress cancels; expiry boots the current selection.
    // Checked before per-key handling so the cancelling key still acts.
    if let Some(deadline) = state.boot_countdown_deadline {
        let key_pressed = ui.ctx().input(|i| {
            i.events.iter().any(|e| {
                matches!(
                    e,
                    egui::Event::Key { pressed: true, .. } | egui::Event::Text(_)
                )
            })
        });
        if key_pressed {
            state.boot_countdown_deadline = None;
        } else if std::time::Instant::now() >= deadline {
            boot_now = true;
        } else {
            // eframe is reactive: keep repainting so the countdown advances
            // without user input.
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(200));
        }
    }

    // Keyboard navigation
    let target_count = state.boot_targets.len();
    ui.ctx().input(|inp| {
        if inp.key_pressed(egui::Key::ArrowDown) && target_count > 0 {
            state.selected_boot_target = (state.selected_boot_target + 1).min(target_count - 1);
        }
        if inp.key_pressed(egui::Key::ArrowUp) {
            state.selected_boot_target = state.selected_boot_target.saturating_sub(1);
        }
        if inp.key_pressed(egui::Key::Enter) && target_count > 0 {
            boot_now = true;
        }
        if inp.key_pressed(egui::Key::F10) {
            *screen = Screen::SelectImage;
        }
        // Esc cancels the countdown (handled above); with no countdown
        // armed it reboots, mirroring the SelectImage screen.
        if inp.key_pressed(egui::Key::Escape) && state.boot_countdown_deadline.is_none() {
            unsafe {
                libc::sync();
                libc::reboot(libc::RB_AUTOBOOT);
            }
        }
    });
    if *screen != Screen::SelectBootTarget {
        return;
    }

    if boot_now {
        let target = &state.boot_targets[state.selected_boot_target];
        match crate::boot_scan::chain_load(target) {
            Ok(id) => {
                tracing::info!(target = %target.label, boot_id = id, "Chain-loading boot target");
                unsafe {
                    libc::sync();
                    libc::reboot(libc::RB_AUTOBOOT);
                }
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to chain-load boot target");
                state.error_message = Some(e.to_string());
                state.boot_countdown_deadline = None;
                return;
            }
        }
    }

    // Hotkeys footer
    egui::Panel::bottom("select_boot_target_hotkeys")
        .frame(egui::Frame::NONE)
        .show_separator_line(false)
        .show_inside(ui, |ui| {
            let hotkeys = vec![
                ("Enter", "Boot"),
                ("F10", "Deploy Images"),
                ("Esc", "Reboot"),
            ];
            widgets::hotkeys::render(ui, &hotkeys, theme);
        });

    egui::CentralPanel::default()
        .frame(egui::Frame::new())
        .show_inside(ui, |ui| {
            ui.vertical(|ui| {
                widgets::header::render(ui, textures, theme);

                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new("Choose an operating system to boot")
                            .color(theme.text_secondary)
                            .strong()
                            .size(16.0),
                    );
                    if let Some(deadline) = state.boot_countdown_deadline {
                        let remaining = deadline
                            .saturating_duration_since(std::time::Instant::now())
                            .as_secs()
                            + 1;
                        let label = &state.boot_targets[state.selected_boot_target].label;
                        ui.label(
                            egui::RichText::new(format!(
                                "Booting {label} in {remaining}s — press any key to cancel"
                            ))
                            .color(theme.accent_gold)
                            .size(14.0),
                        );
                    }
                });

                ui.add_space(10.0);

                // Boot target list with horizontal margins
                let margin = 100.0;
                let list_width = ui.available_width() - margin * 2.0;
                let list_height = ui.available_height();

                let (rect, _) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), list_height),
                    egui::Sense::hover(),
                );

                let mut child = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(egui::Rect::from_min_size(
                            rect.min + egui::vec2(margin, 0.0),
                            egui::vec2(list_width, list_height),
                        ))
                        .layout(egui::Layout::top_down(egui::Align::LEFT)),
                );

                egui::Frame::new()
                    .stroke(egui::Stroke::new(3.0, theme.border.linear_multiply(0.75)))
                    .fill(theme.list_bg)
                    .inner_margin(8.0)
                    .show(&mut child, |ui| {
                        egui::ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                for (i, target) in state.boot_targets.iter().enumerate() {
                                    let is_selected = i == state.selected_boot_target;
                                    let row_fill = if is_selected {
                                        theme.accent_gold.linear_multiply(0.2)
                                    } else {
                                        egui::Color32::TRANSPARENT
                                    };

                                    egui::Frame::new()
                                        .fill(row_fill)
                                        .inner_margin(egui::Margin::symmetric(6, 4))
                                        .corner_radius(4.0)
                                        .show(ui, |ui| {
                                            ui.set_min_width(ui.available_width());
                                            ui.horizontal(|ui| {
                                                // Left column: OS icon or fallback
                                                ui.allocate_ui_with_layout(
                                                    egui::vec2(52.0, 0.0),
                                                    egui::Layout::top_down(egui::Align::Center),
                                                    |ui| {
                                                        let tex = icon_name(&target.loader_dir)
                                                            .and_then(|n| textures.os_icon(n));
                                                        if let Some(tex) = tex {
                                                            ui.add(
                                                                egui::Image::new(tex).max_size(
                                                                    egui::Vec2::splat(32.0),
                                                                ),
                                                            );
                                                        } else {
                                                            ui.label(
                                                                egui::RichText::new("💿")
                                                                    .size(28.0),
                                                            );
                                                        }
                                                    },
                                                );

                                                ui.add_space(10.0);

                                                ui.vertical(|ui| {
                                                    ui.label(
                                                        egui::RichText::new(&target.label)
                                                            .color(theme.text_primary)
                                                            .strong()
                                                            .size(14.0),
                                                    );
                                                    ui.label(
                                                        egui::RichText::new(&target.efi_path)
                                                            .monospace()
                                                            .size(11.0)
                                                            .color(theme.text_secondary),
                                                    );
                                                });
                                            });
                                        });

                                    ui.add_space(2.0);
                                }
                            });
                    });
            });
        });
}
