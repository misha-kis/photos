use std::path::PathBuf;

use crate::photo_library::PhotoLibraryProxy;
use eframe::egui::{self, ColorImage};
use egui::Vec2;
use thumb_size::ThumbSize;

mod photo_library;
pub(crate) mod thumb_size;

enum AppState {
    Main,
    PhotoSelected(usize),
}

struct PhotoLibraryApp {
    photo_library: PhotoLibraryProxy,
    columns: usize,
    first_load: bool,
    state: AppState,
    thumb_size: ThumbSize,
    is_full_photo_requested: bool,
}

impl PhotoLibraryApp {
    fn new() -> Self {
        let gallery_dir = PathBuf::from("/Users/mikhailkiselyov/Pictures/picslib3");
        Self {
            photo_library: PhotoLibraryProxy::new(gallery_dir),
            columns: 2,
            first_load: true,
            state: AppState::Main,
            thumb_size: ThumbSize::T128,
            is_full_photo_requested: false,
        }
    }
}

impl eframe::App for PhotoLibraryApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // self.photo_library.process_loaded_images(ctx);

        egui::CentralPanel::default().show(ctx, |ui| {
            match self.state {
                AppState::PhotoSelected(idx) => {
                    // Full image view
                    // let photo = &self.photo_library.try_get_image(idx as u32);
                    // ui.vertical_centered(|ui| {
                    //     // ui.heading(photo.path.file_name().unwrap().to_string_lossy());
                    //     if ui.button("← Back").clicked() {
                    //         self.state = AppState::Main;
                    //         self.photo_library.full_image_cache = None;
                    //         self.is_full_photo_requested = false;
                    //     }

                    //     // Check if we have the full image cached
                    //     if let Some((cached_path, tex)) = &self.photo_library.full_image_cache {
                    //         if cached_path == &photo.path {
                    //             ui.image(tex);
                    //         }
                    //     } else {
                    //         // Request loading if not cached
                    //         if !self.is_full_photo_requested {
                    //             let _ = self.photo_library.load_tx.send(LoadRequest::FullImage {
                    //                 path: photo.path.clone(),
                    //                 size: ui.available_size(),
                    //             });
                    //             self.is_full_photo_requested = true;
                    //         }
                    //         ui.spinner();
                    //         ui.label("Loading...");
                    //     }
                    // });
                }
                AppState::Main => {
                    self.columns = (ui.clip_rect().width()
                        / (self.thumb_size as u32 as f32 + ui.style().spacing.item_spacing.x)
                            .max(0.0)) as usize;

                    let thumb_height = self.thumb_size as u32 as f32;
                    let total_rows = (self.photo_library.get_number_of_images() + self.columns - 1)
                        / self.columns;

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

                            let mut start_index = start_row * self.columns;
                            let mut end_index = (end_row * self.columns)
                                .min(self.photo_library.get_number_of_images());

                            if start_index > end_index {
                                let x = start_index;
                                start_index = end_index;
                                end_index = x;
                            }

                            // if self.first_load {
                            //     self.first_load = false;
                            // } else {
                            //     for i in start_index..end_index {
                            //         self.photo_library
                            //             .request_thumbnail_load(i, self.thumb_size);
                            //     }
                            // }

                            let mut i = 1;
                            while i <= self.photo_library.get_number_of_images() {
                                ui.horizontal(|ui| {
                                    for _ in 0..self.columns {
                                        if let Some(image) =
                                            self.photo_library.try_get_thumbnail(i as u32)
                                        {
                                            let rgba = image.into_rgba8();
                                            let tex = ctx.load_texture(
                                                format!("thumb-{}", i),
                                                ColorImage::from_rgba_unmultiplied(
                                                    [rgba.width() as _, rgba.height() as _],
                                                    rgba.as_raw(),
                                                ),
                                                Default::default(),
                                            );
                                            if ui
                                                .add(egui::ImageButton::new(&tex).frame(false))
                                                .clicked()
                                            {
                                                self.state = AppState::PhotoSelected(i as usize)
                                            }
                                        } else {
                                            ui.allocate_space(Vec2::new(
                                                self.thumb_size as u32 as f32,
                                                self.thumb_size as u32 as f32,
                                            ));
                                        }
                                    }
                                    i += 1;
                                });
                            }

                            // if ctx.input(|i| i.key_pressed(egui::Key::Period)) {
                            //     self.thumb_size = self.thumb_size.next();
                            // }
                            // if ctx.input(|i| i.key_pressed(egui::Key::Comma)) {
                            //     self.thumb_size = self.thumb_size.prev();
                            // }
                        });
                }
            }
        });
    }
}

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_level(true)
        .with_thread_names(true)
        .with_thread_ids(true)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Photo Library (Lazy Loading)",
        options,
        Box::new(|_| Ok(Box::new(PhotoLibraryApp::new()))),
    )
}
