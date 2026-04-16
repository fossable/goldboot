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
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        // When debug shell is active, inject a synthetic pointer position so
        // egui_term's contains_pointer() check passes. Otherwise filter all
        // pointer events for keyboard-only UI.
        #[cfg(feature = "uki")]
        let debug_shell_active = self.state.debug_shell.is_some();
        #[cfg(not(feature = "uki"))]
        let debug_shell_active = false;

        if debug_shell_active {
            if let Some(screen_rect) = raw_input.screen_rect {
                raw_input.events.push(egui::Event::PointerMoved(screen_rect.center()));
            }
        } else {
            raw_input.events.retain(|e| {
                !matches!(
                    e,
                    egui::Event::PointerMoved(_)
                        | egui::Event::PointerButton { .. }
                        | egui::Event::PointerGone
                        | egui::Event::MouseWheel { .. }
                        | egui::Event::MouseMoved(_)
                )
            });
        }
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.set_cursor_icon(egui::CursorIcon::None);

        self.theme.render_background(ctx);
        self.handle_hotkeys(ctx);

        // Main panel
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
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
            // Esc - Quit/Reboot (only on SelectImage, not if any dialog is open)
            #[cfg(feature = "uki")]
            let debug_shell_active = self.state.debug_shell.is_some();
            #[cfg(not(feature = "uki"))]
            let debug_shell_active = false;

            if i.key_pressed(egui::Key::Escape)
                && !self.state.show_registry_dialog
                && !debug_shell_active
                && self.state.error_message.is_none()
                && self.screen == Screen::SelectImage
            {
                #[cfg(feature = "uki")]
                {
                    // In UKI mode, we're PID 1 so we need to use reboot syscall
                    unsafe {
                        libc::sync();
                        libc::reboot(libc::RB_AUTOBOOT);
                    }
                }
                #[cfg(not(feature = "uki"))]
                std::process::exit(0);
            }

            // F5 - Open registry login dialog
            if i.key_pressed(egui::Key::F5) && self.screen == Screen::SelectImage {
                self.state.show_registry_dialog = true;
            }
        });
    }
}
