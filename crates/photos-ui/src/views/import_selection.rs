use eframe::egui;
use egui_file_dialog::FileDialog;
use std::path::PathBuf;

pub struct ImportSelection {
    file_dialog: FileDialog,
}

impl ImportSelection {
    pub fn new() -> Self {
        let mut file_dialog = FileDialog::new();
        file_dialog.pick_directory();
        Self { file_dialog }
    }

    pub fn show(&mut self, ctx: &egui::Context, on_select: impl FnOnce(PathBuf)) {
        self.file_dialog.update(ctx);
        if let Some(path) = self.file_dialog.take_picked() {
            on_select(path.to_path_buf());
        }
    }
}
