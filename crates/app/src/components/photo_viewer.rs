use crate::components::thumbnail::thumbnail_view;
use crate::photo_library::PhotoLibraryProxy;
use eframe::egui::{self, ColorImage};
use egui::Vec2;
use std::collections::HashMap;

pub struct PhotoViewer {
    texture_handles: HashMap<u32, egui::TextureHandle>,
    image_sizes: HashMap<u32, Vec2>,
}

impl PhotoViewer {
    pub fn new() -> Self {
        Self {
            texture_handles: HashMap::new(),
            image_sizes: HashMap::new(),
        }
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

            let photo_id = photo_index as u32;
            let texture_id = format!("full-image-{}", photo_index);
            let available_size = ui.available_size();

            let cached_image_size = self.image_sizes.get(&photo_id).copied();
            let placeholder_size = available_size.min(Vec2::new(800.0, 600.0));
            let display_size = if let Some(image_size_vec) = cached_image_size {
                let scale = (available_size.x / image_size_vec.x)
                    .min(available_size.y / image_size_vec.y)
                    .min(1.0);
                image_size_vec * scale
            } else {
                placeholder_size
            };

            let try_get_texture = || -> anyhow::Result<Option<egui::TextureHandle>> {
                if let Some(cached_handle) = self.texture_handles.get(&photo_id) {
                    return Ok(Some(cached_handle.clone()));
                }

                match photo_library.try_get_image(photo_id) {
                    Ok(Some(image)) => {
                        let rgba = image.to_rgba8();
                        let size = [rgba.width() as usize, rgba.height() as usize];
                        let image_size_vec = Vec2::new(size[0] as f32, size[1] as f32);
                        let tex = ctx.load_texture(
                            &texture_id,
                            ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()),
                            Default::default(),
                        );
                        self.texture_handles.insert(photo_id, tex.clone());
                        self.image_sizes.insert(photo_id, image_size_vec);
                        Ok(Some(tex))
                    }
                    Ok(None) => Ok(None),
                    Err(e) => Err(e),
                }
            };

            thumbnail_view(
                ui,
                true, // always visible in photo viewer
                (display_size.x, display_size.y),
                try_get_texture,
                None::<fn()>, // no click callback needed
            );
        });
    }
}
