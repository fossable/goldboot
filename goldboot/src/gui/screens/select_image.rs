use super::super::{resources::TextureCache, state::AppState, theme::Theme, widgets};
use super::Screen;
use goldboot_image::ImageArch;
use ubyte::ToByteUnit;

#[cfg(feature = "uki")]
use super::super::state::DebugShell;

fn arch_label(arch: &ImageArch) -> &'static str {
    match arch {
        ImageArch::Amd64 => "x86_64",
        ImageArch::Arm64 => "arm64",
        ImageArch::I386 => "i386",
        ImageArch::Mips => "mips",
        ImageArch::Mips64 => "mips64",
        ImageArch::S390x => "s390x",
    }
}

pub fn render(
    ui: &mut egui::Ui,
    state: &mut AppState,
    textures: &TextureCache,
    theme: &Theme,
    screen: &mut Screen,
) {
    // Debug shell dialog overlay (UKI mode only)
    #[cfg(feature = "uki")]
    if let Some(ref mut shell) = state.debug_shell {
        // Check for PTY exit event
        let mut shell_exited = false;
        while let Ok((_id, event)) = shell.pty_event_receiver.try_recv() {
            if let egui_term::PtyEvent::Exit = event {
                shell_exited = true;
            }
        }

        // Handle Escape to close dialog
        let mut close_shell = false;
        ui.ctx().input(|inp| {
            if inp.key_pressed(egui::Key::Escape) {
                close_shell = true;
            }
        });

        if close_shell || shell_exited {
            state.debug_shell = None;
        } else {
            // Render semi-transparent background overlay
            let screen_rect = ui.ctx().content_rect();
            ui.painter().rect_filled(
                screen_rect,
                0.0,
                egui::Color32::from_black_alpha(180),
            );

            // Render terminal in a centered frame
            egui::Area::new(egui::Id::new("debug_shell_area"))
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .interactable(true)
                .order(egui::Order::Foreground)
                .show(ui.ctx(), |ui| {
                    egui::Frame::new()
                        .fill(egui::Color32::from_rgb(30, 30, 30))
                        .stroke(egui::Stroke::new(2.0, theme.accent_gold))
                        .inner_margin(10.0)
                        .corner_radius(8.0)
                        .show(ui, |ui| {
                            ui.set_min_size(egui::vec2(700.0, 450.0));
                            ui.set_max_size(egui::vec2(900.0, 600.0));

                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new("Debug Shell")
                                            .color(theme.accent_gold)
                                            .strong()
                                            .size(16.0),
                                    );
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            ui.label(
                                                egui::RichText::new("Press Escape to close")
                                                    .color(theme.text_secondary)
                                                    .size(12.0),
                                            );
                                        },
                                    );
                                });
                                ui.add_space(5.0);

                                // Render terminal
                                let available = ui.available_size();
                                let terminal = egui_term::TerminalView::new(ui, &mut shell.terminal_backend)
                                    .set_focus(true)
                                    .set_size(egui::Vec2::new(available.x, available.y - 10.0));
                                ui.add(terminal);
                            });
                        });
                });

            // Request continuous repaint for terminal updates
            ui.ctx().request_repaint();
            return;
        }
    }

    // Hotkeys footer - render first into a bottom panel so it's always visible
    egui::TopBottomPanel::bottom("select_image_hotkeys")
        .frame(egui::Frame::NONE)
        .show_separator_line(false)
        .show_inside(ui, |ui| {
            #[cfg(feature = "uki")]
            let hotkeys = vec![
                ("Esc", "Reboot"),
                ("F5", "Registry Login"),
                ("Enter", "Select Image"),
                ("T", "Debug Shell"),
            ];
            #[cfg(not(feature = "uki"))]
            let hotkeys = vec![
                ("Esc", "Quit"),
                ("F5", "Registry Login"),
                ("Enter", "Select Image"),
            ];
            widgets::hotkeys::render(ui, &hotkeys, theme);
        });

    egui::CentralPanel::default()
        .frame(egui::Frame::new())
        .show_inside(ui, |ui| {
            ui.vertical(|ui| {
                // Header with logo
                widgets::header::render(ui, textures, theme);

                // Prompt
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new("Select an available image below")
                            .color(theme.text_secondary)
                            .strong()
                            .size(16.0),
                    );
                });

                ui.add_space(10.0);

                // Keyboard navigation
                if !state.images.is_empty() {
                    if state.selected_image.is_none() {
                        if let Some(first) = state.images.first() {
                            state.selected_image = Some(first.id.clone());
                        }
                    }

                    let ids: Vec<String> = state.images.iter().map(|i| i.id.clone()).collect();
                    let current_idx = state
                        .selected_image
                        .as_ref()
                        .and_then(|id| ids.iter().position(|i| i == id));

                    ui.ctx().input(|inp| {
                        if inp.key_pressed(egui::Key::ArrowDown) {
                            let next = current_idx.map(|i| (i + 1).min(ids.len() - 1)).unwrap_or(0);
                            state.selected_image = ids.get(next).cloned();
                        }
                        if inp.key_pressed(egui::Key::ArrowUp) {
                            let prev = current_idx.map(|i| i.saturating_sub(1)).unwrap_or(0);
                            state.selected_image = ids.get(prev).cloned();
                        }
                        if inp.key_pressed(egui::Key::Enter) {
                            if state.selected_image.is_some() {
                                *screen = Screen::SelectDevice;
                            }
                        }
                    });
                }

                // Check for T key to open debug shell (works even with no images)
                #[cfg(feature = "uki")]
                {
                    let t_pressed = ui.ctx().input(|inp| {
                        // Check both Key::T and text input for 't' or 'T'
                        inp.key_pressed(egui::Key::T)
                            || inp.events.iter().any(|e| {
                                matches!(e, egui::Event::Text(t) if t == "t" || t == "T")
                            })
                    });

                    // Spawn debug shell if T was pressed
                    if t_pressed && state.debug_shell.is_none() {
                        if let Some(shell) = spawn_debug_shell(ui.ctx()) {
                            state.debug_shell = Some(shell);
                        }
                    }
                }

                // Image list with horizontal margins
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
                                if state.images.is_empty() {
                                    ui.label(
                                        egui::RichText::new("No images found")
                                            .color(theme.text_secondary),
                                    );
                                } else {
                                    for image in state.images.iter() {
                                        let is_selected =
                                            state.selected_image.as_ref() == Some(&image.id);

                                        let size_str = format!(
                                            "{} compressed / {} expanded",
                                            image.file_size.bytes(),
                                            image.primary_header.size.bytes(),
                                        );
                                        let arch_str = arch_label(&image.primary_header.arch);

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
                                                    // Left column: icon above arch badge, left-aligned
                                                    // but items centered on each other within the column
                                                    ui.allocate_ui_with_layout(
                                                        egui::vec2(52.0, 0.0),
                                                        egui::Layout::top_down(egui::Align::Center),
                                                        |ui| {
                                                            let mut any_icon = false;
                                                            for element in
                                                                image.primary_header.elements.iter()
                                                            {
                                                                let os_name = element.os();
                                                                if let Some(tex) =
                                                                    textures.os_icon(&os_name)
                                                                {
                                                                    ui.add(
                                                                        egui::Image::new(tex)
                                                                            .max_size(
                                                                                egui::Vec2::splat(
                                                                                    32.0,
                                                                                ),
                                                                            ),
                                                                    );
                                                                    any_icon = true;
                                                                }
                                                            }
                                                            if !any_icon {
                                                                ui.label(
                                                                    egui::RichText::new("💿")
                                                                        .size(28.0),
                                                                );
                                                            }
                                                            egui::Frame::new()
                                                                .fill(
                                                                    egui::Color32::from_rgb(
                                                                        0x1a, 0x3a, 0x5c,
                                                                    )
                                                                    .linear_multiply(1.5),
                                                                )
                                                                .inner_margin(
                                                                    egui::Margin::symmetric(4, 1),
                                                                )
                                                                .corner_radius(4.0)
                                                                .show(ui, |ui| {
                                                                    ui.label(
                                                                        egui::RichText::new(
                                                                            arch_str,
                                                                        )
                                                                        .color(
                                                                            egui::Color32::from_rgb(
                                                                                0x60, 0xb4, 0xff,
                                                                            ),
                                                                        )
                                                                        .monospace()
                                                                        .size(10.0),
                                                                    );
                                                                });
                                                        },
                                                    );

                                                    ui.add_space(10.0);

                                                    // Right: two lines, left-aligned
                                                    ui.vertical(|ui| {
                                                        ui.label(
                                                            egui::RichText::new(
                                                                image.primary_header.name(),
                                                            )
                                                            .color(theme.text_primary)
                                                            .strong()
                                                            .size(14.0),
                                                        );
                                                        ui.label(
                                                            egui::RichText::new(&size_str)
                                                                .color(theme.text_secondary)
                                                                .monospace()
                                                                .size(11.0),
                                                        );
                                                    });
                                                });
                                            });

                                        ui.add_space(2.0);
                                    }
                                }
                            });
                    });
            });
        });
}

/// Spawn a terminal with busybox shell using egui_term.
#[cfg(feature = "uki")]
fn spawn_debug_shell(ctx: &egui::Context) -> Option<DebugShell> {
    use egui_term::{BackendSettings, TerminalBackend};

    // Create channel for PTY events
    let (pty_event_sender, pty_event_receiver) = std::sync::mpsc::channel();

    // Configure the terminal backend
    let settings = BackendSettings {
        shell: "busybox sh".to_string(),
        ..Default::default()
    };

    // Create the terminal backend
    let terminal_backend = TerminalBackend::new(
        0, // terminal id
        ctx.clone(),
        pty_event_sender,
        settings,
    ).ok()?;

    Some(DebugShell {
        terminal_backend,
        pty_event_receiver,
    })
}
