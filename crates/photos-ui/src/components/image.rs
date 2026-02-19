use eframe::egui::{self, Vec2};

pub fn image_view(
    ui: &mut egui::Ui,
    is_visible: bool,
    size: (f32, f32),
    mut try_get: impl FnMut() -> Option<egui::TextureHandle>,
    click_callback: Option<impl FnMut()>,
) {
    if is_visible {
        match try_get() {
            Some(tex) => {
                let image_size = egui::Vec2::new(size.0, size.1);
                let (rect, response) = ui.allocate_exact_size(image_size, egui::Sense::click());
                let _ = ui.put(rect, |ui: &mut egui::Ui| ui.image((tex.id(), image_size)));
                if response.clicked()
                    && let Some(mut click_callback) = click_callback
                {
                    click_callback()
                }
            }
            None => {
                ui.allocate_space(Vec2::new(size.0, size.1));
            }
        }
    } else {
        ui.allocate_space(Vec2::new(size.0, size.1));
    }
}
