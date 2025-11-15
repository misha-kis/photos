use crate::photo_library::PhotoLibraryProxy;
use eframe::egui::{self, ColorImage};
use photo_library::FaceDetection;

pub struct FacesView {
    desired_face_size: f32,
}

impl FacesView {
    pub fn new() -> Self {
        Self {
            desired_face_size: 100.0,
        }
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        photo_library: &mut PhotoLibraryProxy,
    ) {
        ui.vertical(|ui| {
            // Header with clusterize button
            ui.horizontal(|ui| {
                ui.heading("Faces");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let is_clustering = photo_library.is_clustering_in_progress();
                    let button_text = if is_clustering {
                        "Clusterizing..."
                    } else {
                        "Clusterize"
                    };
                    
                    let button = ui.add_enabled(!is_clustering, egui::Button::new(button_text));
                    if button.clicked() {
                        photo_library.start_clusterization();
                    }
                });
            });

            ui.separator();

            // Display faces
            if let Some(faces_map) = photo_library.get_faces_grouped_by_id() {
                if faces_map.is_empty() {
                    ui.centered_and_justified(|ui| {
                        ui.label("No faces found. Click 'Clusterize' to cluster detected faces.");
                    });
                } else {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false; 2])
                        .show(ui, |ui| {
                            let mut face_ids: Vec<u32> = faces_map.keys().copied().collect();
                            face_ids.sort();

                            for face_id in face_ids {
                                let detections = &faces_map[&face_id];
                                
                                ui.group(|ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(format!("Face ID: {} ({} detections)", face_id, detections.len()));
                                    });
                                    
                                    ui.separator();
                                    
                                    // Display face thumbnails in a grid
                                    let available_width = ui.available_width();
                                    let spacing = ui.style().spacing.item_spacing.x;
                                    let columns = ((available_width + spacing) / (self.desired_face_size + spacing))
                                        .floor()
                                        .max(1.0) as usize;
                                    
                                    let actual_face_size = ((available_width + spacing) / columns as f32 - spacing)
                                        .max(50.0)
                                        .min(200.0);
                                    
                                    let mut i = 0;
                                    while i < detections.len() {
                                        ui.horizontal(|ui| {
                                            for _ in 0..columns {
                                                if i < detections.len() {
                                                    let detection = &detections[i];
                                                    self.show_face_thumbnail(
                                                        ui,
                                                        ctx,
                                                        photo_library,
                                                        detection,
                                                        actual_face_size,
                                                    );
                                                } else {
                                                    ui.allocate_space(egui::Vec2::new(
                                                        actual_face_size,
                                                        actual_face_size,
                                                    ));
                                                }
                                                i += 1;
                                            }
                                        });
                                    }
                                });
                                
                                ui.add_space(10.0);
                            }
                        });
                }
            } else {
                ui.centered_and_justified(|ui| {
                    ui.label("No faces clustered yet. Click 'Clusterize' to start.");
                });
            }
        });
    }

    fn show_face_thumbnail(
        &self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        photo_library: &mut PhotoLibraryProxy,
        detection: &FaceDetection,
        size: f32,
    ) {
        if let Some(image) = photo_library.try_get_face_thumbnail(detection.detection_id) {
            let rgba = image.into_rgba8();
            let tex = ctx.load_texture(
                format!("face-{}", detection.detection_id),
                ColorImage::from_rgba_unmultiplied(
                    [rgba.width() as _, rgba.height() as _],
                    rgba.as_raw(),
                ),
                Default::default(),
            );
            let image_size = egui::Vec2::new(size, size);
            ui.image((tex.id(), image_size));
        } else {
            ui.allocate_space(egui::Vec2::new(size, size));
        }
    }
}

