use crate::app_proxy::AppProxy;
use crate::components::image::image_view;
use eframe::egui;
use photos_core::Uuid;
use std::collections::HashMap;

const THUMBNAIL_SIZE: f32 = 128.0;

pub struct FacesView {
    /// Detection id -> texture handle for face thumbnails
    texture_handles: HashMap<Uuid, egui::TextureHandle>,
    should_update_clusters: bool,
}

impl FacesView {
    pub fn new() -> Self {
        Self {
            texture_handles: HashMap::new(),
            should_update_clusters: true,
        }
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        app_proxy: &mut AppProxy,
        mut on_item_clicked: impl FnMut(usize),
    ) {
        app_proxy.process_events();

        if self.should_update_clusters {
            self.should_update_clusters = false;
            app_proxy.refresh_face_clusters();
        }
        let face_clusters = app_proxy.face_clusters.clone();

        let mut item_index = 0_usize;
        egui::ScrollArea::vertical()
            .id_salt("faces_vertical")
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                for (cluster_index, (cluster_id, detection_ids)) in face_clusters.iter().enumerate()
                {
                    if cluster_index > 0 {
                        ui.add(egui::Separator::default().horizontal());
                    }

                    let row_height = THUMBNAIL_SIZE + ui.style().spacing.item_spacing.y;
                    ui.push_id(*cluster_id, |ui| {
                        egui::ScrollArea::horizontal()
                            .id_salt("face_row")
                            .auto_shrink([false; 2])
                            .max_height(row_height)
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    for detection_id in detection_ids {
                                        let is_visible = true;
                                        let texture_opt = app_proxy
                                            .get_face_detection_thumbnail(detection_id, ctx);
                                        let current_index = item_index;
                                        let mut click_cb = || on_item_clicked(current_index);
                                        ui.push_id(*detection_id, |ui| {
                                            image_view(
                                                ui,
                                                is_visible,
                                                (THUMBNAIL_SIZE, THUMBNAIL_SIZE),
                                                || Ok(texture_opt.clone()),
                                                Some(&mut click_cb),
                                            );
                                        });
                                        item_index += 1;
                                    }
                                });
                            });
                    });
                }
            });
    }
}
