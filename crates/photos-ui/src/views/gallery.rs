use crate::app_proxy::AppProxy;
use crate::components::dynamic_grid::DynamicGrid;
use crate::components::image::image_view;
use eframe::egui;
use photos_domain::ImageId;
use std::collections::HashMap;

pub struct GalleryView {
    texture_handles: HashMap<ImageId, egui::TextureHandle>,
    dynamic_grid: DynamicGrid<ImageId, egui::TextureHandle>,
}

impl GalleryView {
    pub fn new() -> Self {
        Self {
            texture_handles: HashMap::new(),
            dynamic_grid: DynamicGrid::new(128.0),
        }
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        app_proxy: &mut AppProxy,
        on_item_clicked: impl FnMut(usize),
    ) {
        app_proxy.process_events();
        
        let image_ids = app_proxy.image_ids.clone();
        for image_id in &image_ids {
            if !self.texture_handles.contains_key(image_id) {
                if let Some(image) = app_proxy.get_cached_thumbnail(image_id) {
                    let rgba = image.clone().into_rgba8();
                    let texture_id = format!("thumbnail-{}", image_id);
                    let tex = ctx.load_texture(
                        &texture_id,
                        egui::ColorImage::from_rgba_unmultiplied(
                            [rgba.width() as _, rgba.height() as _],
                            rgba.as_raw(),
                        ),
                        Default::default(),
                    );
                    self.texture_handles.insert(*image_id, tex);
                }
            }
        }

        let get_item_data = |image_id: &ImageId| -> Option<egui::TextureHandle> {
            if let Some(cached_handle) = self.texture_handles.get(image_id) {
                return Some(cached_handle.clone());
            }

            app_proxy.request_thumbnail(*image_id);
            None
        };

        self.dynamic_grid.show(
            ui,
            ctx,
            &image_ids,
            get_item_data,
            |ui, visible, size, texture_opt, click| {
                image_view(ui, visible, size, || Ok(texture_opt.clone()), Some(click));
            },
            on_item_clicked,
        );
    }
}
