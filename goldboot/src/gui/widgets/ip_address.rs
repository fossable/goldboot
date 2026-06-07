use super::super::{state::AppState, theme::Theme};

pub fn render(ctx: &egui::Context, state: &AppState, theme: &Theme) {
    egui::Area::new(egui::Id::new("ip_address_overlay"))
        .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-10.0, -10.0))
        .order(egui::Order::Foreground)
        .interactable(false)
        .show(ctx, |ui| {
            ui.with_layout(egui::Layout::top_down(egui::Align::RIGHT), |ui| {
                if state.ip_addresses.is_empty() {
                    ui.label(
                        egui::RichText::new("offline")
                            .color(egui::Color32::from_rgb(0xd6, 0x3b, 0x3b))
                            .monospace()
                            .strong()
                            .size(12.0),
                    );
                } else {
                    for ip in &state.ip_addresses {
                        ui.label(
                            egui::RichText::new(ip)
                                .color(theme.text_secondary)
                                .monospace()
                                .size(12.0),
                        );
                    }
                }
            });
        });
}
