use eframe::egui;
use egui::{ColorImage, TextureHandle, Vec2};
use std::path::PathBuf;

struct Photo {
    path: PathBuf,
    thumbnail: TextureHandle,
}

struct PhotoLibraryApp {
    photos: Vec<Photo>,
    scroll_to_end: bool,
    selected_photo: Option<usize>,
}

impl PhotoLibraryApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // In a real app, load from filesystem:
        let dummy_images = vec!["../../test_data/example.png"; 50];

        let mut photos = Vec::new();
        for name in dummy_images {
            // Placeholder solid-color thumbnail
            let color_image = ColorImage::example();
            let texture =
                cc.egui_ctx
                    .load_texture(name.to_string(), color_image, Default::default());
            photos.push(Photo {
                path: PathBuf::from(name),
                thumbnail: texture,
            });
        }

        Self {
            photos,
            scroll_to_end: true,
            selected_photo: None,
        }
    }
}

impl eframe::App for PhotoLibraryApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(idx) = self.selected_photo {
                // Full-size photo view
                ui.vertical_centered(|ui| {
                    ui.heading("Viewing Photo");
                    if ui.button("Back to Library").clicked() {
                        self.selected_photo = None;
                    }

                    let photo = &self.photos[idx];
                    let image_size = Vec2::new(512.0, 512.0);
                    ui.image(
                        &photo.thumbnail,
                        // image_size,
                    );
                });
            } else {
                // Scrollable grid of thumbnails
                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .stick_to_bottom(true) // start at end
                    .show(ui, |ui| {
                        let columns = 3;
                        let thumb_size = Vec2::new(64.0, 64.0);
                        let mut row = 0;

                        for (i, photo) in self.photos.iter().enumerate() {
                            if row % columns == 0 {
                                ui.horizontal(|ui| {
                                    for j in 0..columns {
                                        if let Some(photo) = self.photos.get(i + j) {
                                            if ui
                                                .add(egui::ImageButton::new(
                                                    &photo.thumbnail,
                                                    // thumb_size,
                                                ))
                                                .clicked()
                                            {
                                                self.selected_photo = Some(i + j);
                                            }
                                        }
                                    }
                                });
                            }
                            row += 1;
                        }
                    });
            }
        });
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Photo Library",
        options,
        Box::new(|cc| Ok(Box::new(PhotoLibraryApp::new(cc)))),
    )
}
