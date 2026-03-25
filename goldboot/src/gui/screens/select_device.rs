use super::super::{resources::TextureCache, state::AppState, theme::Theme, widgets};
use super::Screen;
use ubyte::ToByteUnit;

pub fn render(
    ui: &mut egui::Ui,
    state: &mut AppState,
    textures: &TextureCache,
    theme: &Theme,
    screen: &mut Screen,
) {
    // Hotkeys footer - render first into a bottom panel so it's always visible
    egui::TopBottomPanel::bottom("select_device_hotkeys")
        .frame(egui::Frame::NONE)
        .show_separator_line(false)
        .show_inside(ui, |ui| {
            let hotkeys = vec![("Esc", "Back"), ("Enter", "Overwrite")];
            widgets::hotkeys::render(ui, &hotkeys, theme);
        });

    egui::CentralPanel::default()
        .frame(egui::Frame::new())
        .show_inside(ui, |ui| {
            ui.vertical(|ui| {
                // Header with logo
                widgets::header::render(ui, textures, theme);

                // Warning prompt
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new("Select a device below to OVERWRITE")
                            .color(theme.text_secondary)
                            .strong()
                            .size(16.0),
                    );
                });

                ui.add_space(10.0);

                // Keyboard navigation
                if !state.devices.is_empty() {
                    // Auto-select first device if nothing selected
                    let required_bytes: Option<u64> = state
                        .selected_image
                        .as_ref()
                        .and_then(|id| state.images.iter().find(|i| &i.id == id))
                        .map(|i| i.primary_header.size);

                    // Only selectable devices (sufficient capacity)
                    let selectable: Vec<&block_utils::Device> = state
                        .devices
                        .iter()
                        .filter(|d| required_bytes.map(|req| d.capacity >= req).unwrap_or(true))
                        .collect();

                    if state.selected_device.is_none() {
                        if let Some(&first) = selectable.first() {
                            state.selected_device = Some(first.clone());
                        }
                    }

                    let current_idx = state
                        .selected_device
                        .as_ref()
                        .and_then(|sel| selectable.iter().position(|d| d.name == sel.name));

                    ui.ctx().input(|inp| {
                        if inp.key_pressed(egui::Key::ArrowDown) {
                            let next = current_idx
                                .map(|i| (i + 1).min(selectable.len() - 1))
                                .unwrap_or(0);
                            state.selected_device = selectable.get(next).map(|d| (*d).clone());
                        }
                        if inp.key_pressed(egui::Key::ArrowUp) {
                            let prev = current_idx.map(|i| i.saturating_sub(1)).unwrap_or(0);
                            state.selected_device = selectable.get(prev).map(|d| (*d).clone());
                        }
                        if inp.key_pressed(egui::Key::Enter) {
                            if state.selected_device.is_some() {
                                *screen = Screen::Confirm;
                            }
                        }
                        if inp.key_pressed(egui::Key::Escape) {
                            *screen = Screen::SelectImage;
                        }
                    });
                }

                // Device list with horizontal margins
                let margin = 100.0;
                let list_width = ui.available_width() - margin * 2.0;
                let list_height = ui.available_height();

                ui.add_space(0.0); // flush layout
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
                                if state.devices.is_empty() {
                                    ui.label(
                                        egui::RichText::new("No devices found")
                                            .color(theme.text_secondary),
                                    );
                                } else {
                                    // Required capacity from selected image
                                    let required_bytes: Option<u64> = state
                                        .selected_image
                                        .as_ref()
                                        .and_then(|id| state.images.iter().find(|i| &i.id == id))
                                        .map(|i| i.primary_header.size);

                                    for device in state.devices.iter() {
                                        let too_small = required_bytes
                                            .map(|req| device.capacity < req)
                                            .unwrap_or(false);

                                        let device_path = format!("/dev/{}", device.name);
                                        let is_selected = !too_small
                                            && state
                                                .selected_device
                                                .as_ref()
                                                .map(|sel| sel.name == device.name)
                                                .unwrap_or(false);

                                        let icon = match device.media_type {
                                            block_utils::MediaType::SolidState => {
                                                &textures.icon_ssd
                                            }
                                            block_utils::MediaType::Rotational => {
                                                &textures.icon_hdd
                                            }
                                            block_utils::MediaType::NVME => &textures.icon_nvme,
                                            block_utils::MediaType::Ram => &textures.icon_ram,
                                            _ => &textures.icon_hdd,
                                        };

                                        let name_line = if let Some(serial) = &device.serial_number
                                        {
                                            format!("{} ({})", device_path, serial)
                                        } else {
                                            device_path.clone()
                                        };
                                        let size_line = device.capacity.bytes().to_string();

                                        let row_fill = if is_selected {
                                            theme.accent_gold.linear_multiply(0.2)
                                        } else {
                                            egui::Color32::TRANSPARENT
                                        };
                                        let text_color = if too_small {
                                            theme.text_secondary.linear_multiply(0.5)
                                        } else {
                                            theme.text_primary
                                        };
                                        let icon_tint = if too_small { 0.3 } else { 1.0 };

                                        egui::Frame::new()
                                            .fill(row_fill)
                                            .inner_margin(egui::Margin::symmetric(6, 4))
                                            .corner_radius(4.0)
                                            .show(ui, |ui| {
                                                ui.set_min_width(ui.available_width());
                                                ui.horizontal(|ui| {
                                                    // Left column: icon centered within fixed width
                                                    ui.allocate_ui_with_layout(
                                                        egui::vec2(52.0, 0.0),
                                                        egui::Layout::top_down(egui::Align::Center),
                                                        |ui| {
                                                            ui.add(
                                                                egui::Image::new(icon)
                                                                    .max_size(egui::Vec2::splat(
                                                                        32.0,
                                                                    ))
                                                                    .tint(
                                                                        egui::Color32::WHITE
                                                                            .linear_multiply(
                                                                                icon_tint,
                                                                            ),
                                                                    ),
                                                            );
                                                        },
                                                    );

                                                    ui.add_space(10.0);

                                                    // Right: two lines, left-aligned
                                                    ui.vertical(|ui| {
                                                        ui.label(
                                                            egui::RichText::new(&name_line)
                                                                .color(text_color)
                                                                .strong()
                                                                .size(14.0),
                                                        );
                                                        let mut size_label =
                                                            egui::RichText::new(&size_line)
                                                                .monospace()
                                                                .size(11.0);
                                                        size_label = if too_small {
                                                            size_label
                                                                .color(egui::Color32::DARK_RED)
                                                        } else {
                                                            size_label.color(theme.text_secondary)
                                                        };
                                                        ui.label(size_label);
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
