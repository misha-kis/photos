use crate::app_proxy::AppProxy;
use crate::components::image::image_view;
use eframe::egui;
use photos_domain::ImageId;
use std::collections::HashMap;

pub struct GalleryView {
    pub n_columns: usize,
    desired_image_size: f32,
    texture_handles: HashMap<ImageId, egui::TextureHandle>,
}

impl GalleryView {
    pub fn new() -> Self {
        Self {
            n_columns: 2,
            desired_image_size: 128.0,
            texture_handles: HashMap::new(),
        }
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        app_proxy: &mut AppProxy,
        mut on_photo_selected: impl FnMut(usize),
    ) {
        let available_width = ui.available_width();
        let spacing = ui.style().spacing.item_spacing.x;

        self.n_columns = ((available_width + spacing) / (self.desired_image_size + spacing))
            .floor()
            .max(1.0) as usize;

        let actual_image_size =
            ((available_width + spacing) / self.n_columns as f32 - spacing).clamp(50.0, 500.0);

        let thumb_height = actual_image_size;
        let total_rows = (app_proxy.number_of_images() + self.n_columns - 1) / self.n_columns;

        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                let clip_rect = ui.clip_rect();
                let scroll_y = ui.min_rect().top();
                let visible_start = ((clip_rect.top() - scroll_y)
                    / (thumb_height + ui.style().spacing.item_spacing.y))
                    .floor() as isize;
                let visible_end = ((clip_rect.bottom() - scroll_y)
                    / (thumb_height + ui.style().spacing.item_spacing.y))
                    .ceil() as isize;

                let margin = 1;
                let start_row = (visible_start - margin).max(0) as usize;
                let end_row = ((visible_end + margin) as usize).min(total_rows);

                let mut start_index = start_row * self.n_columns;
                let mut end_index = (end_row * self.n_columns).min(app_proxy.number_of_images());

                if start_index > end_index {
                    let x = start_index;
                    start_index = end_index;
                    end_index = x;
                }

                let mut i = 1;
                for chunk in app_proxy.image_ids.as_slice().chunks(self.n_columns) {
                    ui.horizontal(|ui| {
                        for image_id in chunk {
                            let is_visible = start_index <= i && i <= end_index;
                            let texture_id = format!("thumbnail-{}", image_id);

                            let click_callback = || on_photo_selected(i as usize);

                            let try_get_texture =
                                || -> anyhow::Result<Option<egui::TextureHandle>> {
                                    if let Some(cached_handle) = self.texture_handles.get(&image_id)
                                    {
                                        return Ok(Some(cached_handle.clone()));
                                    }

                                    match app_proxy.try_get_thumbnail(*image_id) {
                                        Ok(Some(image)) => {
                                            let rgba = image.into_rgba8();
                                            let tex = ctx.load_texture(
                                                &texture_id,
                                                egui::ColorImage::from_rgba_unmultiplied(
                                                    [rgba.width() as _, rgba.height() as _],
                                                    rgba.as_raw(),
                                                ),
                                                Default::default(),
                                            );
                                            self.texture_handles.insert(*image_id, tex.clone());
                                            Ok(Some(tex))
                                        }
                                        Ok(None) => Ok(None),
                                        Err(e) => Err(e),
                                    }
                                };

                            image_view(
                                ui,
                                is_visible,
                                (actual_image_size, actual_image_size),
                                try_get_texture,
                                Some(click_callback),
                            );
                            i += 1;
                        }
                    });
                }
            });
    }
}
