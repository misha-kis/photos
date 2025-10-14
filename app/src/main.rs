use crate::photo_library::{LoadRequest, PhotoLibrary};
use eframe::egui;
use egui::Vec2;

mod photo_library;

struct PhotoLibraryApp {
    photo_library: photo_library::PhotoLibrary,
    columns: usize,
    first_load: bool,
}

impl PhotoLibraryApp {
    fn new() -> Self {
        let dir = dirs::picture_dir().unwrap().join("picslib");
        Self {
            photo_library: PhotoLibrary::new(dir),
            columns: 2,
            first_load: true,
        }
    }
}

impl eframe::App for PhotoLibraryApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Process any loaded images from the worker thread
        self.photo_library.process_loaded_images(ctx);

        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(idx) = self.photo_library.selected_photo {
                // Full image view
                let photo = &self.photo_library.photos[idx];
                ui.vertical_centered(|ui| {
                    ui.heading(photo.path.file_name().unwrap().to_string_lossy());
                    if ui.button("← Back").clicked() {
                        self.photo_library.selected_photo = None;
                        self.photo_library.full_image_cache = None;
                    }

                    // Check if we have the full image cached
                    if let Some((cached_path, tex)) = &self.photo_library.full_image_cache {
                        if cached_path == &photo.path {
                            ui.image(tex);
                        }
                    } else {
                        // Request loading if not cached
                        let _ = self.photo_library.load_tx.send(LoadRequest::FullImage {
                            path: photo.path.clone(),
                        });
                        ui.spinner();
                        ui.label("Loading...");
                    }
                });
                return;
            }

            self.columns = (ui.clip_rect().width()
                / (self.photo_library.thumb_size.x + ui.style().spacing.item_spacing.x).max(0.0))
                as usize;

            // Thumbnail grid with lazy loading
            let thumb_height = self.photo_library.thumb_size.y;
            let total_rows = (self.photo_library.photos.len() + self.columns - 1) / self.columns;

            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    // Compute visible range using scroll area clip rect
                    let clip_rect = ui.clip_rect();
                    let scroll_y = ui.min_rect().top();
                    println!("scroll: {}", scroll_y);
                    let visible_start = ((clip_rect.top() - scroll_y)
                        / (thumb_height + ui.style().spacing.item_spacing.y))
                        .floor() as isize;
                    let visible_end = ((clip_rect.bottom() - scroll_y)
                        / (thumb_height + ui.style().spacing.item_spacing.y))
                        .ceil() as isize;

                    // Load thumbnails near visible region (with margin)
                    let margin = 1;
                    let start_row = (visible_start - margin).max(0) as usize;
                    let end_row = ((visible_end + margin) as usize).min(total_rows);

                    let mut start_index = start_row * self.columns;
                    let mut end_index =
                        (end_row * self.columns).min(self.photo_library.photos.len());

                    println!("{start_index} - {end_index}");
                    if start_index > end_index {
                        let x = start_index;
                        start_index = end_index;
                        end_index = x;
                    }

                    if self.first_load {
                        self.first_load = false;
                    } else {
                        for i in start_index..end_index {
                            self.photo_library.request_thumbnail_load(i);
                        }
                    }

                    // Now draw all (using placeholders if not loaded)
                    // let mut i = self.photos.len() as i32;
                    let mut i = 0;
                    while i < self.photo_library.photos.len() {
                        ui.horizontal(|ui| {
                            for _ in 0..self.columns {
                                if let Some(photo) = self.photo_library.photos.get(i as usize) {
                                    if let Some(tex) = &photo.thumbnail {
                                        if ui
                                            .add(egui::ImageButton::new(tex).frame(false))
                                            .clicked()
                                        {
                                            self.photo_library.selected_photo = Some(i as usize);
                                        }
                                    } else {
                                        ui.allocate_space(Vec2::new(200f32, 200f32));
                                    }
                                }
                                i += 1;
                            }
                        });
                        // ui.add_space(4.0);
                    }

                    // Add total height spacer so scroll behaves correctly
                    // ui.set_min_height(total_height);
                });
        });
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Photo Library (Lazy Loading)",
        options,
        Box::new(|_| Ok(Box::new(PhotoLibraryApp::new()))),
    )
}
