use super::super::theme::Theme;

/// Renders an error dialog overlay. Returns `true` if the dialog was shown and should block
/// further rendering, `false` if no error or the dialog was dismissed.
pub fn render(ui: &mut egui::Ui, error_message: &mut Option<String>, theme: &Theme) -> bool {
    let Some(error) = error_message.as_ref() else {
        return false;
    };

    let close = ui.ctx().input(|inp| {
        inp.key_pressed(egui::Key::Escape) || inp.key_pressed(egui::Key::Enter)
    });

    if close {
        *error_message = None;
        return false;
    }

    ui.painter().rect_filled(
        ui.ctx().content_rect(),
        0.0,
        egui::Color32::from_black_alpha(180),
    );

    egui::Area::new(egui::Id::new("error_dialog"))
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .order(egui::Order::Foreground)
        .show(ui.ctx(), |ui| {
            egui::Frame::new()
                .fill(egui::Color32::from_rgb(40, 30, 30))
                .stroke(egui::Stroke::new(2.0, egui::Color32::from_rgb(200, 80, 80)))
                .inner_margin(20.0)
                .corner_radius(8.0)
                .show(ui, |ui| {
                    ui.set_max_width(500.0);
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new("Error")
                                .color(egui::Color32::from_rgb(200, 80, 80))
                                .strong()
                                .size(18.0),
                        );
                        ui.add_space(10.0);
                        ui.label(
                            egui::RichText::new(error)
                                .color(theme.text_primary)
                                .size(14.0),
                        );
                        ui.add_space(15.0);
                        ui.label(
                            egui::RichText::new("Press Escape or Enter to close")
                                .color(theme.text_secondary)
                                .size(12.0),
                        );
                    });
                });
        });

    true
}
