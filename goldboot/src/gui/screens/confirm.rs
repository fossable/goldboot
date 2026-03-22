use super::super::{resources::TextureCache, state::AppState, theme::Theme, widgets};
use super::Screen;
use goldboot_image::ImageArch;
use ubyte::ToByteUnit;

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

fn media_type_label(mt: &block_utils::MediaType) -> &'static str {
    match mt {
        block_utils::MediaType::SolidState => "SSD",
        block_utils::MediaType::Rotational => "HDD",
        block_utils::MediaType::NVME => "NVMe",
        block_utils::MediaType::Ram => "RAM",
        _ => "Unknown",
    }
}

pub fn render(
    ui: &mut egui::Ui,
    state: &mut AppState,
    textures: &TextureCache,
    theme: &Theme,
    screen: &mut Screen,
) {
    // Hotkeys footer - render first into a bottom panel so it's always visible
    egui::TopBottomPanel::bottom("confirm_hotkeys")
        .frame(egui::Frame::NONE)
        .show_separator_line(false)
        .show_inside(ui, |ui| {
            let confirm_key_label = format!("'{}'", state.confirm_char);
            let hotkeys = vec![("Esc", "Quit"), (&*confirm_key_label, "Confirm (x100)")];
            widgets::hotkeys::render(ui, &hotkeys, theme);
        });

    egui::CentralPanel::default()
        .frame(egui::Frame::new())
        .show_inside(ui, |ui| {
            ui.vertical(|ui| {
                // Header with logo
                widgets::header::render(ui, textures, theme);

                // Warning
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new("Are you sure?")
                            .color(egui::Color32::from_rgb(0xff, 0x60, 0x60))
                            .strong()
                            .size(16.0),
                    );
                });

                ui.add_space(16.0);

                // Summary panels
                ui.horizontal(|ui| {
                    ui.add_space(100.0);

                    let available_width = ui.available_width() - 200.0;
                    let panel_width = (available_width - 16.0) / 2.0;

                    // Image info panel
                    egui::Frame::new()
                        .stroke(egui::Stroke::new(3.0, theme.border.linear_multiply(0.75)))
                        .fill(theme.list_bg)
                        .inner_margin(14.0)
                        .show(ui, |ui| {
                            ui.set_width(panel_width);

                            ui.label(
                                egui::RichText::new("Image")
                                    .color(theme.accent_gold)
                                    .strong()
                                    .size(13.0),
                            );
                            ui.add_space(8.0);

                            if let Some(image_id) = &state.selected_image.clone() {
                                if let Some(image) =
                                    state.images.iter().find(|i| &i.id == image_id)
                                {
                                    let h = &image.primary_header;

                                    // OS icons + name
                                    ui.horizontal(|ui| {
                                        for element in h.elements.iter() {
                                            let os_name = element.os();
                                            if let Some(tex) = textures.os_icon(&os_name) {
                                                ui.add(
                                                    egui::Image::new(tex)
                                                        .max_size(egui::Vec2::splat(24.0)),
                                                );
                                            }
                                        }
                                        ui.label(
                                            egui::RichText::new(h.name())
                                                .color(theme.text_primary)
                                                .strong()
                                                .size(14.0),
                                        );
                                    });

                                    ui.add_space(8.0);

                                    let rows: &[(&str, String)] = &[
                                        ("Architecture", arch_label(&h.arch).to_string()),
                                        (
                                            "Size on disk",
                                            format!("{}", image.file_size.bytes()),
                                        ),
                                        (
                                            "Expanded size",
                                            format!("{}", h.size.bytes()),
                                        ),
                                        (
                                            "Elements",
                                            h.elements
                                                .iter()
                                                .map(|e| e.name())
                                                .collect::<Vec<_>>()
                                                .join(", "),
                                        ),
                                        (
                                            "Encryption",
                                            format!("{:?}", h.encryption_type),
                                        ),
                                        ("ID", image.id[..12].to_string()),
                                    ];

                                    egui::Grid::new("image_info_grid")
                                        .num_columns(2)
                                        .spacing([12.0, 4.0])
                                        .show(ui, |ui| {
                                            for (label, value) in rows {
                                                ui.label(
                                                    egui::RichText::new(*label)
                                                        .color(theme.text_secondary)
                                                        .size(12.0),
                                                );
                                                ui.label(
                                                    egui::RichText::new(value)
                                                        .color(theme.text_primary)
                                                        .monospace()
                                                        .size(12.0),
                                                );
                                                ui.end_row();
                                            }
                                        });
                                } else {
                                    ui.label(
                                        egui::RichText::new("Image not found")
                                            .color(theme.text_secondary),
                                    );
                                }
                            } else {
                                ui.label(
                                    egui::RichText::new("No image selected")
                                        .color(theme.text_secondary),
                                );
                            }
                        });

                    ui.add_space(16.0);

                    // Device info panel
                    egui::Frame::new()
                        .stroke(egui::Stroke::new(3.0, theme.border.linear_multiply(0.75)))
                        .fill(theme.list_bg)
                        .inner_margin(14.0)
                        .show(ui, |ui| {
                            ui.set_width(panel_width);

                            ui.label(
                                egui::RichText::new("Target Device")
                                    .color(theme.accent_gold)
                                    .strong()
                                    .size(13.0),
                            );
                            ui.add_space(8.0);

                            if let Some(device_name) = &state.selected_device.clone() {
                                if let Some(device) =
                                    state.devices.iter().find(|d| &d.name == device_name)
                                {
                                    // Device icon + name
                                    ui.horizontal(|ui| {
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
                                        ui.add(egui::Image::new(icon).max_width(24.0));
                                        ui.label(
                                            egui::RichText::new(&device.name)
                                                .color(theme.text_primary)
                                                .strong()
                                                .size(14.0),
                                        );
                                    });

                                    ui.add_space(8.0);

                                    let serial = device
                                        .serial_number
                                        .as_deref()
                                        .unwrap_or("—")
                                        .to_string();
                                    let lbs = device
                                        .logical_block_size
                                        .map(|b| format!("{}", b.bytes()))
                                        .unwrap_or_else(|| "—".to_string());
                                    let pbs = device
                                        .physical_block_size
                                        .map(|b| format!("{}", b.bytes()))
                                        .unwrap_or_else(|| "—".to_string());

                                    let rows: &[(&str, String)] = &[
                                        ("Capacity", format!("{}", device.capacity.bytes())),
                                        ("Type", media_type_label(&device.media_type).to_string()),
                                        ("Filesystem", format!("{:?}", device.fs_type)),
                                        ("Serial", serial),
                                        ("Logical block", lbs),
                                        ("Physical block", pbs),
                                    ];

                                    egui::Grid::new("device_info_grid")
                                        .num_columns(2)
                                        .spacing([12.0, 4.0])
                                        .show(ui, |ui| {
                                            for (label, value) in rows {
                                                ui.label(
                                                    egui::RichText::new(*label)
                                                        .color(theme.text_secondary)
                                                        .size(12.0),
                                                );
                                                ui.label(
                                                    egui::RichText::new(value)
                                                        .color(theme.text_primary)
                                                        .monospace()
                                                        .size(12.0),
                                                );
                                                ui.end_row();
                                            }
                                        });
                                } else {
                                    ui.label(
                                        egui::RichText::new("Device not found")
                                            .color(theme.text_secondary),
                                    );
                                }
                            } else {
                                ui.label(
                                    egui::RichText::new("No device selected")
                                        .color(theme.text_secondary),
                                );
                            }
                        });

                    ui.add_space(100.0);
                });

                ui.add_space(16.0);

                // Progress bar (centered)
                ui.vertical_centered(|ui| {
                    let progress_text = format!("{}%", (state.confirm_progress * 100.0) as i32);

                    let progress_bar = egui::ProgressBar::new(state.confirm_progress)
                        .show_percentage()
                        .text(progress_text);

                    ui.add_sized([400.0, 20.0], progress_bar);

                    ui.add_space(10.0);

                    ui.label(
                        egui::RichText::new(format!(
                            "Press '{}' 100 times to confirm",
                            state.confirm_char
                        ))
                        .color(theme.text_secondary)
                        .size(12.0),
                    );
                });

                // Check for confirm char press
                let confirm_char = state.confirm_char;
                if ui.input(|i| {
                    i.events
                        .iter()
                        .any(|e| matches!(e, egui::Event::Text(t) if t.chars().next() == Some(confirm_char)))
                }) {
                    state.confirm_progress += 0.01;
                    if state.confirm_progress >= 1.0 {
                        state.confirm_progress = 1.0;
                        state.init_write_progress(10 * 1024 * 1024 * 1024);
                        *screen = Screen::ApplyImage;
                    }
                }
            });
        });
}
