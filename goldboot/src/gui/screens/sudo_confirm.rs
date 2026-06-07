use super::super::{state::AppState, theme::Theme};

pub fn render(ctx: &egui::Context, state: &mut AppState, _theme: &Theme) {
    if !state.show_sudo_dialog {
        return;
    }

    egui::Window::new("Elevated Permissions Required")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label("Elevated permissions are required to write images.");
            ui.add_space(8.0);
            ui.label("Re-invoke with sudo?");
            ui.add_space(16.0);

            let reinvoke = ui.input(|i| {
                i.events
                    .iter()
                    .any(|e| matches!(e, egui::Event::Text(t) if t.eq_ignore_ascii_case("y")))
            });

            ui.horizontal(|ui| {
                if ui.button("Yes (sudo)").clicked() || reinvoke {
                    let args: Vec<String> = std::env::args().collect();
                    let _ = std::process::Command::new("sudo").args(&args).status();
                    std::process::exit(0);
                }
                if ui.button("No").clicked() {
                    state.show_sudo_dialog = false;
                }
            });
        });

    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        state.show_sudo_dialog = false;
    }
}
