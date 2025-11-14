use std::path::PathBuf;

use crate::components::{gallery_view::GalleryView, photo_viewer::PhotoViewer};
use crate::photo_library::PhotoLibraryProxy;
use eframe::egui;
use thumb_size::ThumbSize;

mod components;
mod photo_library;
pub(crate) mod thumb_size;

enum AppState {
    Main,
    PhotoSelected(usize),
}

struct PhotoLibraryApp {
    photo_library: PhotoLibraryProxy,
    state: AppState,
    is_full_photo_requested: bool,
    gallery_view: GalleryView,
    photo_viewer: PhotoViewer,
}

impl PhotoLibraryApp {
    fn new() -> Self {
        let gallery_dir = PathBuf::from("/Users/misha-kis/Pictures/picslib3");
        Self {
            photo_library: PhotoLibraryProxy::new(gallery_dir),
            state: AppState::Main,
            is_full_photo_requested: false,
            gallery_view: GalleryView::new(ThumbSize::T128),
            photo_viewer: PhotoViewer::new(),
        }
    }
}

impl eframe::App for PhotoLibraryApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            match self.state {
                AppState::PhotoSelected(idx) => {
                    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                        self.state = AppState::Main;
                        self.is_full_photo_requested = false;
                    }
                    
                    let total_images = self.photo_library.get_number_of_images();
                    if ctx.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
                        if idx > 1 {
                            self.state = AppState::PhotoSelected(idx - 1);
                            self.is_full_photo_requested = false;
                        }
                    }
                    if ctx.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
                        if idx < total_images {
                            self.state = AppState::PhotoSelected(idx + 1);
                            self.is_full_photo_requested = false;
                        }
                    }
                    
                    self.photo_viewer.show(
                        ui,
                        ctx,
                        &mut self.photo_library,
                        idx,
                        || {
                            self.state = AppState::Main;
                            self.is_full_photo_requested = false;
                        },
                    );
                }
                AppState::Main => {
                    self.gallery_view.show(
                        ui,
                        ctx,
                        &mut self.photo_library,
                        |idx| {
                            self.state = AppState::PhotoSelected(idx);
                        },
                    );
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
