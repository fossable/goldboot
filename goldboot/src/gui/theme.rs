use egui::{Color32, Style, Visuals};

pub struct Theme {
    pub bg_primary: Color32,    // #333333
    pub bg_grid: Color32,       // #4a4a4a
    pub accent_gold: Color32,   // #c8ab37
    pub text_primary: Color32,  // #ffffff (white)
    pub text_secondary: Color32, // #aea79f (beige)
    pub list_bg: Color32,       // #333333 with 0.75 opacity
    pub border: Color32,        // #c8ab37
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            bg_primary: Color32::from_rgb(0x33, 0x33, 0x33),
            bg_grid: Color32::from_rgb(0x4a, 0x4a, 0x4a),
            accent_gold: Color32::from_rgb(0xc8, 0xab, 0x37),
            text_primary: Color32::WHITE,
            text_secondary: Color32::from_rgb(0xae, 0xa7, 0x9f),
            list_bg: Color32::from_rgba_unmultiplied(0x33, 0x33, 0x33, 191),
            border: Color32::from_rgb(0xc8, 0xab, 0x37),
        }
    }
}

impl Theme {
    pub fn apply_to_context(&self, ctx: &egui::Context) {
        let mut style = Style::default();
        style.visuals = Visuals::dark();
        style.visuals.override_text_color = Some(self.text_primary);
        style.visuals.selection.bg_fill = self.accent_gold;
        style.visuals.selection.stroke.color = self.accent_gold;
        style.visuals.widgets.noninteractive.bg_stroke.color = self.border;
        ctx.set_style(style);
    }

    pub fn render_background(&self, ctx: &egui::Context) {
        let painter = ctx.layer_painter(egui::LayerId::background());
        let rect = ctx.screen_rect();

        // Fill with primary background color
        painter.rect_filled(rect, 0.0, self.bg_primary);

        // Draw 80x80px grid
        let grid_size = 80.0;
        let stroke = egui::Stroke::new(1.0, self.bg_grid);

        // Vertical lines
        let mut x = (rect.min.x / grid_size).floor() * grid_size;
        while x <= rect.max.x {
            painter.line_segment(
                [egui::pos2(x, rect.min.y), egui::pos2(x, rect.max.y)],
                stroke,
            );
            x += grid_size;
        }

        // Horizontal lines
        let mut y = (rect.min.y / grid_size).floor() * grid_size;
        while y <= rect.max.y {
            painter.line_segment(
                [egui::pos2(rect.min.x, y), egui::pos2(rect.max.x, y)],
                stroke,
            );
            y += grid_size;
        }
    }
}
