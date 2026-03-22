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

pub fn render(
    ui: &mut egui::Ui,
    state: &mut AppState,
    textures: &TextureCache,
    theme: &Theme,
    screen: &mut Screen,
) {
    // Hotkeys footer - render first into a bottom panel so it's always visible
    egui::TopBottomPanel::bottom("select_image_hotkeys")
        .frame(egui::Frame::NONE)
        .show_separator_line(false)
        .show_inside(ui, |ui| {
            let hotkeys = vec![
                ("Esc", "Quit"),
                ("F5", "Registry Login"),
                ("Enter", "Select Image"),
            ];
            widgets::hotkeys::render(ui, &hotkeys, theme);
        });

    // Main content
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

                // Image list with horizontal margins (100px as per GTK)
                ui.horizontal(|ui| {
                    ui.add_space(100.0);

                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.push_id("image_list", |ui| {
                                let available_width = ui.available_width() - 200.0;

                                egui::Frame::new()
                                    .stroke(egui::Stroke::new(
                                        3.0,
                                        theme.border.linear_multiply(0.75),
                                    ))
                                    .fill(theme.list_bg)
                                    .inner_margin(15.0)
                                    .show(ui, |ui| {
                                        ui.set_width(available_width);

                                        if state.images.is_empty() {
                                            ui.label(
                                                egui::RichText::new("No images found")
                                                    .color(theme.text_secondary),
                                            );
                                        } else {
                                            for image in state.images.iter() {
                                                let is_selected = state.selected_image.as_ref()
                                                    == Some(&image.id);

                                                let response = ui.horizontal(|ui| {
                                                    ui.add_space(5.0);

                                                    // All OS icons side by side
                                                    let mut any_icon = false;
                                                    for element in
                                                        image.primary_header.elements.iter()
                                                    {
                                                        let os_name = element.os();
                                                        if let Some(tex) =
                                                            textures.os_icon(&os_name)
                                                        {
                                                            ui.add(
                                                                egui::Image::new(tex).max_size(
                                                                    egui::Vec2::splat(32.0),
                                                                ),
                                                            );
                                                            any_icon = true;
                                                        }
                                                    }
                                                    if !any_icon {
                                                        ui.label(
                                                            egui::RichText::new("💿").size(24.0),
                                                        );
                                                    }

                                                    ui.add_space(8.0);

                                                    // Image name
                                                    ui.label(
                                                        egui::RichText::new(
                                                            image.primary_header.name(),
                                                        )
                                                        .color(theme.text_primary)
                                                        .strong()
                                                        .size(15.0),
                                                    );

                                                    ui.with_layout(
                                                        egui::Layout::right_to_left(
                                                            egui::Align::Center,
                                                        ),
                                                        |ui| {
                                                            ui.add_space(5.0);

                                                            // Arch badge (right-aligned)
                                                            let arch_str = arch_label(
                                                                &image.primary_header.arch,
                                                            );
                                                            egui::Frame::new()
                                                                .fill(
                                                                    egui::Color32::from_rgb(
                                                                        0x1a, 0x3a, 0x5c,
                                                                    )
                                                                    .linear_multiply(1.5),
                                                                )
                                                                .inner_margin(
                                                                    egui::Margin::symmetric(6, 2),
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
                                                                        .size(12.0),
                                                                    );
                                                                });

                                                            ui.add_space(12.0);

                                                            // Image size: actual / expanded
                                                            let size_str = format!(
                                                                "{} / {}",
                                                                image.file_size.bytes(),
                                                                image.primary_header.size.bytes(),
                                                            );
                                                            ui.label(
                                                                egui::RichText::new(size_str)
                                                                    .color(theme.text_secondary)
                                                                    .monospace()
                                                                    .size(12.0),
                                                            );
                                                        },
                                                    );
                                                });

                                                let response = response
                                                    .response
                                                    .interact(egui::Sense::click());

                                                if response.clicked() {
                                                    state.selected_image =
                                                        Some(image.id.clone());
                                                    *screen = Screen::SelectDevice;
                                                }

                                                if response.hovered() {
                                                    ui.ctx().set_cursor_icon(
                                                        egui::CursorIcon::PointingHand,
                                                    );
                                                }

                                                // Check for Enter key to select
                                                if is_selected
                                                    && ui.input(|i| {
                                                        i.key_pressed(egui::Key::Enter)
                                                    })
                                                {
                                                    *screen = Screen::SelectDevice;
                                                }

                                                ui.add_space(5.0);
                                            }
                                        }
                                    });
                            });
                        });

                    ui.add_space(100.0);
                });
            });
        });
}
