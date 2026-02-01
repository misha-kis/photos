use crate::app_proxy::AppProxy;
use crate::components::dynamic_grid::DynamicGrid;
use crate::components::image::image_view;
use eframe::egui;
use photos_core::Uuid;
use std::collections::HashMap;

pub struct FacesView {
    texture_handles: HashMap<Uuid, egui::TextureHandle>,
    dynamic_grid: DynamicGrid<Uuid, egui::TextureHandle>,
    should_update_face_ids: bool,
}

impl FacesView {
    pub fn new() -> Self {
        Self {
            texture_handles: HashMap::new(),
            dynamic_grid: DynamicGrid::new(128.0),
            should_update_face_ids: true,
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

        if self.should_update_face_ids {
            self.should_update_face_ids = false;
            app_proxy.refresh_face_ids();
        }
        let face_ids = app_proxy.face_ids.clone();

        for face_id in &face_ids {
            if !self.texture_handles.contains_key(face_id)
                && let Some(image) = app_proxy.get_cached_face_thumbnail(face_id)
            {
                let rgba = image.clone().into_rgba8();
                let texture_id = format!("thumbnail-{}", face_id);
                let tex = ctx.load_texture(
                    &texture_id,
                    egui::ColorImage::from_rgba_unmultiplied(
                        [rgba.width() as _, rgba.height() as _],
                        rgba.as_raw(),
                    ),
                    Default::default(),
                );
                self.texture_handles.insert(*face_id, tex);
            }
        }

        let get_item_data = |face_uuid: &Uuid| -> Option<egui::TextureHandle> {
            if let Some(cached_handle) = self.texture_handles.get(face_uuid) {
                return Some(cached_handle.clone());
            }

            app_proxy.request_face_thumbnail(*face_uuid);
            None
        };

        self.dynamic_grid.show(
            ui,
            &face_ids,
            get_item_data,
            |ui, visible, size, texture_opt, click| {
                image_view(ui, visible, size, || Ok(texture_opt.clone()), Some(click));
            },
            on_item_clicked,
            false,
        );
    }
}
