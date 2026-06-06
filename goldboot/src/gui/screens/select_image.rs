use super::super::{
    resources::TextureCache,
    state::{AppState, SelectedImage},
    theme::Theme,
    widgets,
};
use super::Screen;
use goldboot_image::ImageArch;
use ubyte::ToByteUnit;

/// One entry in the merged local + registry image list.
enum DisplayItem {
    Local {
        id: String,
        name: String,
        size_str: String,
        arch: ImageArch,
        os: Option<String>,
    },
    Registry {
        host: String,
        name: String,
        tag: String,
        size_str: String,
        arch: ImageArch,
    },
}

impl DisplayItem {
    fn as_selected(&self) -> SelectedImage {
        match self {
            DisplayItem::Local { id, .. } => SelectedImage::Local(id.clone()),
            DisplayItem::Registry {
                host, name, tag, ..
            } => SelectedImage::Registry {
                host: host.clone(),
                name: name.clone(),
                tag: tag.clone(),
            },
        }
    }

    fn matches(&self, sel: &SelectedImage) -> bool {
        match (self, sel) {
            (DisplayItem::Local { id, .. }, SelectedImage::Local(s)) => id == s,
            (
                DisplayItem::Registry {
                    host: h,
                    name: n,
                    tag: t,
                    ..
                },
                SelectedImage::Registry {
                    host: sh,
                    name: sn,
                    tag: st,
                },
            ) => h == sh && n == sn && t == st,
            _ => false,
        }
    }
}

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
    // Error dialog overlay
    if widgets::error_dialog::render(ui, &mut state.error_message, theme) {
        return;
    }

    // Debug shell overlay (UKI mode only)
    #[cfg(feature = "uki")]
    {
        if let Some(ref mut shell) = state.debug_shell {
            let mut shell_exited = false;
            while let Ok((_id, event)) = shell.pty_event_receiver.try_recv() {
                if let egui_term::PtyEvent::Exit = event {
                    shell_exited = true;
                }
            }

            // Handle Escape to close dialog
            let close_shell = ui.ctx().input(|inp| inp.key_pressed(egui::Key::Escape));

            if close_shell || shell_exited {
                state.debug_shell = None;
            } else {
                // Render semi-transparent background overlay
                ui.painter().rect_filled(
                    ui.ctx().content_rect(),
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
                                    let terminal = egui_term::TerminalView::new(
                                        ui,
                                        &mut shell.terminal_backend,
                                    )
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
    }

    // Hotkeys footer
    egui::Panel::bottom("select_image_hotkeys")
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

                // Build merged display list: local images then registry
                // images. The order is stable across frames so keyboard
                // navigation is predictable.
                let registry_host = if state.registry_address.is_empty() {
                    "registry".to_string()
                } else {
                    state.registry_address.clone()
                };
                let mut items: Vec<DisplayItem> = Vec::new();
                for img in state.images.iter() {
                    items.push(DisplayItem::Local {
                        id: img.id.clone(),
                        name: img.primary_header.name_str(),
                        size_str: format!(
                            "{} compressed / {} expanded",
                            img.file_size.bytes(),
                            img.primary_header.size.bytes(),
                        ),
                        arch: img.primary_header.arch,
                        os: img.primary_header.elements.first().map(|e| e.os()),
                    });
                }
                for entry in state.registry_images.iter() {
                    items.push(DisplayItem::Registry {
                        host: registry_host.clone(),
                        name: entry.name.clone(),
                        tag: entry.tag.clone(),
                        size_str: format!("{} expanded", entry.size.bytes()),
                        arch: entry.arch,
                    });
                }

                // Keyboard navigation across the merged list
                if !items.is_empty() {
                    if state.selected_image.is_none() {
                        state.selected_image = Some(items[0].as_selected());
                    }
                    let current_idx = state
                        .selected_image
                        .as_ref()
                        .and_then(|sel| items.iter().position(|d| d.matches(sel)));

                    ui.ctx().input(|inp| {
                        if inp.key_pressed(egui::Key::ArrowDown) {
                            let next = current_idx
                                .map(|i| (i + 1).min(items.len() - 1))
                                .unwrap_or(0);
                            state.selected_image = items.get(next).map(|d| d.as_selected());
                        }
                        if inp.key_pressed(egui::Key::ArrowUp) {
                            let prev = current_idx.map(|i| i.saturating_sub(1)).unwrap_or(0);
                            state.selected_image = items.get(prev).map(|d| d.as_selected());
                        }
                        if inp.key_pressed(egui::Key::Enter) && state.selected_image.is_some() {
                            *screen = Screen::SelectDevice;
                        }
                    });
                }

                // T key opens debug shell (UKI mode only)
                #[cfg(feature = "uki")]
                if ui.ctx().input(|inp| inp.key_pressed(egui::Key::T))
                    && state.debug_shell.is_none()
                {
                    match spawn_debug_shell(ui.ctx()) {
                        Ok(shell) => state.debug_shell = Some(shell),
                        Err(e) => {
                            state.error_message = Some(format!("Failed to open debug shell: {e}"))
                        }
                    }
                }

                // Image list
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
                                if items.is_empty() {
                                    let msg = if state.registry_list_loading {
                                        "Loading registry…"
                                    } else {
                                        "No images found. Press F5 to log in to a registry."
                                    };
                                    ui.label(egui::RichText::new(msg).color(theme.text_secondary));
                                } else {
                                    for item in items.iter() {
                                        let is_selected = state
                                            .selected_image
                                            .as_ref()
                                            .map(|s| item.matches(s))
                                            .unwrap_or(false);

                                        let (name, size_str, arch, os, source_label) = match item {
                                            DisplayItem::Local {
                                                name,
                                                size_str,
                                                arch,
                                                os,
                                                ..
                                            } => (
                                                name.clone(),
                                                size_str.clone(),
                                                *arch,
                                                os.clone(),
                                                None,
                                            ),
                                            DisplayItem::Registry {
                                                name,
                                                tag,
                                                size_str,
                                                arch,
                                                host,
                                                ..
                                            } => (
                                                format!("{name}:{tag}"),
                                                size_str.clone(),
                                                *arch,
                                                None,
                                                Some(host.clone()),
                                            ),
                                        };
                                        let arch_str = arch_label(&arch);

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
                                                    ui.allocate_ui_with_layout(
                                                        egui::vec2(52.0, 0.0),
                                                        egui::Layout::top_down(egui::Align::Center),
                                                        |ui| {
                                                            let mut any_icon = false;
                                                            if let Some(os_name) = &os {
                                                                if let Some(tex) =
                                                                    textures.os_icon(os_name)
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

                                                    ui.vertical(|ui| {
                                                        ui.horizontal(|ui| {
                                                            ui.label(
                                                                egui::RichText::new(&name)
                                                                    .color(theme.text_primary)
                                                                    .strong()
                                                                    .size(14.0),
                                                            );
                                                            if let Some(src) = &source_label {
                                                                ui.label(
                                                                    egui::RichText::new(format!(
                                                                        "registry · {src}"
                                                                    ))
                                                                    .color(theme.accent_gold)
                                                                    .monospace()
                                                                    .size(10.0),
                                                                );
                                                            }
                                                        });
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
fn spawn_debug_shell(ctx: &egui::Context) -> Result<DebugShell, anyhow::Error> {
    use egui_term::{BackendSettings, TerminalBackend};

    let (pty_event_sender, pty_event_receiver) = std::sync::mpsc::channel();

    // Find a working shell - try $SHELL first, then common paths
    let shell = std::env::var("SHELL")
        .ok()
        .filter(|s| std::path::Path::new(s).exists())
        .or_else(|| {
            ["/bin/sh", "/bin/bash", "/bin/busybox"]
                .iter()
                .find(|p| std::path::Path::new(p).exists())
                .map(|s| s.to_string())
        })
        .ok_or_else(|| anyhow::anyhow!("No shell found at /bin/sh, /bin/bash, or /bin/busybox"))?;

    let terminal_backend = TerminalBackend::new(
        0,
        ctx.clone(),
        pty_event_sender,
        BackendSettings {
            shell,
            ..Default::default()
        },
    )?;

    Ok(DebugShell {
        terminal_backend,
        pty_event_receiver,
    })
}
