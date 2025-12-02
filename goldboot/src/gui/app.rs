use super::{
    resources::TextureCache,
    screens::{registry_login, Screen},
    state::AppState,
    theme::Theme,
};

pub struct GuiApp {
    pub screen: Screen,
    pub state: AppState,
    pub theme: Theme,
    pub textures: TextureCache,
}

impl GuiApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let theme = Theme::default();
        theme.apply_to_context(&cc.egui_ctx);

        Self {
            screen: Screen::SelectImage,
            state: AppState::new(),
            theme,
            textures: TextureCache::new(&cc.egui_ctx),
        }
    }
}

impl eframe::App for GuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Render grid background
        self.theme.render_background(ctx);

        // Handle global hotkeys
        self.handle_hotkeys(ctx);

        // Main panel
        egui::CentralPanel::default()
            .frame(egui::Frame::none())
            .show(ctx, |ui| {
                self.screen
                    .render(ui, &mut self.state, &self.textures, &self.theme);
            });

        // Render registry login dialog if open
        registry_login::render(ctx, &mut self.state, &self.theme);
    }
}

impl GuiApp {
    fn handle_hotkeys(&mut self, ctx: &egui::Context) {
        ctx.input(|i| {
            // Esc - Quit application
            if i.key_pressed(egui::Key::Escape) && !self.state.show_registry_dialog {
                std::process::exit(0);
            }

            // F5 - Open registry login dialog
            if i.key_pressed(egui::Key::F5) && self.screen == Screen::SelectImage {
                self.state.show_registry_dialog = true;
            }
        });
    }
}
