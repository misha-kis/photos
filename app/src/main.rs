use std::path::PathBuf;

use crate::components::{
    faces_view::FacesView, gallery_view::GalleryView, import_view::ImportView, navbar::Navbar,
    photo_viewer::PhotoViewer,
};
use crate::photo_library::PhotoLibraryProxy;
use eframe::egui;

mod components;
mod photo_library;

enum AppState {
    Gallery,
    PhotoSelected(usize),
    Import,
    Faces,
}

struct PhotoLibraryApp {
    photo_library: PhotoLibraryProxy,
    state: AppState,
    is_full_photo_requested: bool,
    gallery_view: GalleryView,
    photo_viewer: PhotoViewer,
    navbar: Navbar,
    import_view: ImportView,
    faces_view: FacesView,
    pending_file_dialog: bool,
}

impl PhotoLibraryApp {
    fn new() -> Self {
        let picture_dir = dirs::picture_dir().unwrap();
        let gallery_dir = picture_dir.join("picslib3");
        Self {
            photo_library: PhotoLibraryProxy::new(gallery_dir),
            state: AppState::Gallery,
            is_full_photo_requested: false,
            gallery_view: GalleryView::new(),
            photo_viewer: PhotoViewer::new(),
            navbar: Navbar::new(),
            import_view: ImportView::new(),
            faces_view: FacesView::new(),
            pending_file_dialog: false,
        }
    }

    fn open_file_dialog(&mut self) {
        self.pending_file_dialog = true;
    }

    fn handle_file_dialog(&mut self) {
        if !self.pending_file_dialog {
            return;
        }
        self.pending_file_dialog = false;

        let initial_dir = dirs::picture_dir().unwrap_or_else(|| PathBuf::from("/"));

        let files = rfd::FileDialog::new()
            .add_filter(
                "Images",
                &["jpg", "jpeg", "png", "gif", "bmp", "webp", "tiff", "tif"],
            )
            .set_directory(&initial_dir)
            .pick_files();

        if let Some(selected_files) = files {
            self.import_view.set_files(selected_files);
            self.state = AppState::Import;
            return;
        }
    }
}

impl eframe::App for PhotoLibraryApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.pending_file_dialog {
            self.handle_file_dialog();
        }

        egui::SidePanel::left("navbar")
            .resizable(false)
            .default_width(150.0)
            .show(ctx, |ui| {
                if let Some(action) = self.navbar.show(ui) {
                    match action {
                        crate::components::navbar::NavAction::Gallery => {
                            self.state = AppState::Gallery;
                        }
                        crate::components::navbar::NavAction::Faces => {
                            self.state = AppState::Faces;
                        }
                        crate::components::navbar::NavAction::Import => {
                            self.open_file_dialog();
                        }
                    }
                }
            });

        egui::CentralPanel::default().show(ctx, |ui| match self.state {
            AppState::PhotoSelected(idx) => {
                if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.state = AppState::Gallery;
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

                self.photo_viewer
                    .show(ui, ctx, &mut self.photo_library, idx, || {
                        self.state = AppState::Gallery;
                        self.is_full_photo_requested = false;
                    });
            }
            AppState::Gallery => {
                self.gallery_view
                    .show(ui, ctx, &mut self.photo_library, |idx| {
                        self.state = AppState::PhotoSelected(idx);
                    });
            }
            AppState::Import => {
                let files_to_import = self.import_view.files().to_vec();
                let mut should_cancel = false;
                let mut should_import = false;

                self.import_view.show(
                    ui,
                    ctx,
                    || {
                        should_cancel = true;
                    },
                    |_files| {
                        should_import = true;
                    },
                );

                if should_cancel {
                    self.state = AppState::Gallery;
                    self.import_view.set_files(Vec::new());
                } else if should_import {
                    for file in &files_to_import {
                        if let Err(e) = self.photo_library.import_photo(file.clone()) {
                            eprintln!("Failed to import {:?}: {}", file, e);
                        }
                    }
                    self.photo_library.refresh_image_count();
                    self.state = AppState::Gallery;
                    self.import_view.set_files(Vec::new());
                }
            }
            AppState::Faces => {
                self.faces_view.show(ui, ctx, &mut self.photo_library);
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
