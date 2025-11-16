use crate::photo_library::PhotoLibraryProxy;
use eframe::egui::{self, ColorImage};
use photo_library::{FaceDetection, FaceThumbnail};

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
            if let Some(faces) = photo_library.get_unique_face_thumbnails() {
                
                    egui::ScrollArea::vertical()
                        .auto_shrink([false; 2])
                        .show(ui, |ui| {
                            for face in faces {
                                self.show_face_thumbnail(ui, ctx, photo_library, &face, self.desired_face_size);
                            }
                        });
                
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
        face: &FaceThumbnail,
        size: f32,
    ) {
            let rgba = face.thumbnail.to_rgba8();
            let tex = ctx.load_texture(
                format!("face-{}", face.face_detection_id),
                ColorImage::from_rgba_unmultiplied(
                    [rgba.width() as usize, rgba.height() as usize],
                    rgba.as_raw(),
                ),
                Default::default(),
            );
            let image_size = egui::Vec2::new(size, size);
            ui.image((tex.id(), image_size));
    }
}

