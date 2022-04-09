use gtk::prelude::*;
use gtk::*;
use gtk4 as gtk;

pub struct AbortView {
    pub container: Grid,
    pub progress: ProgressBar,
}

impl AbortView {
    pub fn new() -> Self {
        let progress = ProgressBar::new();
        progress.set_text(Some("Progress Bar"));
        progress.set_show_text(true);
        progress.set_hexpand(true);

        let container = Grid::new();
        container.attach(&progress, 0, 0, 1, 1);
        container.set_row_spacing(12);
        container.set_vexpand(true);
        container.set_hexpand(true);

        Self {
            container,
            progress,
        }
    }
}
