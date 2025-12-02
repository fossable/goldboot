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
    // Load devices if not already loaded
    if state.devices.is_empty() {
        if let Ok(block_devices) = block_utils::get_block_devices() {
            if let Ok(devices) = block_utils::get_all_device_info(block_devices) {
                state.devices = devices;
            }
        }
    }

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

        // Device list with horizontal margins (100px as per GTK)
        ui.horizontal(|ui| {
            ui.add_space(100.0);

            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.push_id("device_list", |ui| {
                        let available_width = ui.available_width() - 200.0;

                        egui::Frame::none()
                            .stroke(egui::Stroke::new(3.0, theme.border.linear_multiply(0.75)))
                            .fill(theme.list_bg)
                            .inner_margin(15.0)
                            .show(ui, |ui| {
                                ui.set_width(available_width);

                                if state.devices.is_empty() {
                                    ui.label(
                                        egui::RichText::new("No devices found")
                                            .color(theme.text_secondary),
                                    );
                                } else {
                                    for device in state.devices.iter() {
                                        let is_selected =
                                            state.selected_device.as_ref() == Some(&device.name);

                                        let response = ui.horizontal(|ui| {
                                            ui.add_space(5.0);

                                            // Device icon (32x32)
                                            let icon = match device.media_type {
                                                block_utils::MediaType::SolidState => &textures.icon_ssd,
                                                block_utils::MediaType::Rotational => &textures.icon_hdd,
                                                block_utils::MediaType::NVME => &textures.icon_nvme,
                                                block_utils::MediaType::Ram => &textures.icon_ram,
                                                _ => &textures.icon_hdd, // Fallback
                                            };

                                            ui.add(egui::Image::new(icon).max_width(32.0));
                                            ui.add_space(5.0);

                                            // Device name and serial
                                            let device_label = if let Some(serial) = &device.serial_number {
                                                format!("{} ({})", device.name, serial)
                                            } else {
                                                device.name.clone()
                                            };

                                            ui.label(
                                                egui::RichText::new(device_label)
                                                    .color(theme.text_primary),
                                            );

                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    ui.add_space(5.0);

                                                    // Device capacity
                                                    ui.label(
                                                        egui::RichText::new(device.capacity.bytes().to_string())
                                                            .color(theme.text_primary),
                                                    );
                                                },
                                            );
                                        });

                                        let response = response.response.interact(egui::Sense::click());

                                        if response.clicked() {
                                            state.selected_device = Some(device.name.clone());
                                            // Navigate to Confirm screen
                                            *screen = Screen::Confirm;
                                        }

                                        if response.hovered() {
                                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                        }

                                        // Check for Enter key to select
                                        if is_selected && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                            *screen = Screen::Confirm;
                                        }

                                        ui.add_space(5.0);
                                    }
                                }
                            });
                    });
                });

            ui.add_space(100.0);
        });

        ui.add_space(20.0);

        // Hotkeys footer
        let hotkeys = vec![("Esc", "Quit"), ("Enter", "Overwrite")];
        widgets::hotkeys::render(ui, &hotkeys, theme);
    });
}
