use super::{
    resources::TextureCache,
    screens::{Screen, registry_login},
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
    pub fn new(cc: &eframe::CreationContext<'_>, needs_sudo: bool) -> Self {
        let theme = Theme::default();
        theme.apply_to_context(&cc.egui_ctx);

        Self {
            screen: if needs_sudo {
                Screen::SudoConfirm
            } else {
                Screen::SelectImage
            },
            state: AppState::new(),
            theme,
            textures: TextureCache::new(&cc.egui_ctx),
        }
    }
}

impl eframe::App for GuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Disable all pointer/mouse input
        ctx.input_mut(|i| {
            i.pointer = Default::default();
            i.events.retain(|e| !matches!(e,
                egui::Event::PointerMoved(_)
                | egui::Event::PointerButton { .. }
                | egui::Event::PointerGone
                | egui::Event::MouseWheel { .. }
                | egui::Event::MouseMoved(_)
            ));
        });

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
