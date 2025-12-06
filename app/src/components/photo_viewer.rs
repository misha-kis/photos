use crate::photo_library::PhotoLibraryProxy;
use eframe::egui::{self, ColorImage};
use egui::Vec2;

pub struct PhotoViewer;

impl PhotoViewer {
    pub fn new() -> Self {
        Self
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        photo_library: &mut PhotoLibraryProxy,
        photo_index: usize,
        on_back: impl FnOnce(),
    ) {
        ui.vertical_centered(|ui| {
            if ui.button("← Back to Gallery").clicked() {
                on_back();
            }
            ui.add_space(10.0);

            if let Some(image) = photo_library.try_get_image(photo_index as u32) {
                let rgba = image.to_rgba8();
                let size = [rgba.width() as usize, rgba.height() as usize];

                let tex = ctx.load_texture(
                    format!("full-image-{}", photo_index),
                    ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()),
                    Default::default(),
                );

                let available_size = ui.available_size();
                let image_size = Vec2::new(size[0] as f32, size[1] as f32);
                let scale = (available_size.x / image_size.x)
                    .min(available_size.y / image_size.y)
                    .min(1.0);
                let display_size = image_size * scale;

                ui.image((tex.id(), display_size));
            } else {
                ui.spinner();
                ui.label("Loading full image...");
            }
        });
    }
}
