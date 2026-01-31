mod app_proxy;
mod components;
mod views;

use std::path::PathBuf;

use crate::components::navbar::{NavAction, show_navbar};
use crate::views::faces::FacesView;
use eframe::egui;
use photos_app::config::Options;
use views::gallery::GalleryView;
use views::import::ImportView;

const LAST_PATH_KEY: &'static str = "last_path";

pub enum InitializedAppState {
    Gallery(GalleryView),
    Faces(FacesView),
    Import(ImportView),
}

struct InitializedApp {
    app_proxy: app_proxy::AppProxy,
    state: InitializedAppState,
}

impl InitializedApp {
    fn new(path: PathBuf) -> anyhow::Result<Self> {
        let app_options = Options::default();
        let app_proxy = app_proxy::AppProxy::new(path, app_options)?;
        let state = InitializedAppState::Gallery(GalleryView::new());
        Ok(Self { app_proxy, state })
    }
}

impl InitializedApp {
    fn ui(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame, should_close: &mut bool) {
        egui::SidePanel::left("navbar")
            .resizable(false)
            .default_width(150.0)
            .min_width(20.0)
            .show(ctx, |ui| {
                if let Some(action) = show_navbar(ui) {
                    match action {
                        NavAction::Gallery => {
                            self.state = InitializedAppState::Gallery(GalleryView::new());
                        }
                        NavAction::Faces => {
                            self.state = InitializedAppState::Faces(FacesView::new());
                        }
                        NavAction::Import => {
                            self.state = InitializedAppState::Import(ImportView::new())
                        }
                        NavAction::Close => *should_close = true,
                    }
                }
            });

        egui::CentralPanel::default().show(ctx, |ui| match &mut self.state {
            InitializedAppState::Gallery(view) => view.show(ui, ctx, &mut self.app_proxy, |_| {}),
            InitializedAppState::Faces(view) => view.show(ui, ctx, &mut self.app_proxy, |_| {}),
            InitializedAppState::Import(view) => {
                let mut new_state = None;
                view.show(ui, ctx, &mut self.app_proxy, || {
                    new_state = Some(InitializedAppState::Gallery(GalleryView::new()))
                });
                if let Some(state) = new_state {
                    self.state = state;
                    self.app_proxy.refresh_images();
                }
            }
        });
    }
}

struct UiApp {
    app: Option<InitializedApp>,
}

impl eframe::App for UiApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        match &mut self.app {
            None => {
                self.path_picker_ui(ctx, frame);
            }
            Some(app) => {
                let mut should_close = false;
                app.ui(ctx, frame, &mut should_close);
                if should_close {
                    self.app = None;
                    if let Some(storage) = frame.storage_mut() {
                        storage.set_string(LAST_PATH_KEY, String::default());
                    }
                }
            }
        }
    }
}

impl UiApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut saved_path = cc.storage.and_then(|s| s.get_string(LAST_PATH_KEY));
        if saved_path.as_ref().is_some_and(|s| s.is_empty()) {
            saved_path = None;
        }

        let app = if let Some(path) = saved_path {
            let path = PathBuf::from(path);

            match InitializedApp::new(path) {
                Ok(app) => Some(app),
                Err(_) => None,
            }
        } else {
            None
        };

        Self { app }
    }
    fn path_picker_ui(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.heading("Where do you want to store your photos?");

                if ui.button("Choose Folder").clicked()
                    && let Some(path) = rfd::FileDialog::new().pick_folder()
                {
                    self.initialize_with_path(path, frame);
                }
            });
        });
    }

    fn initialize_with_path(&mut self, path: PathBuf, frame: &mut eframe::Frame) {
        match InitializedApp::new(path.clone()) {
            Ok(app) => {
                if let Some(storage) = frame.storage_mut() {
                    storage.set_string(LAST_PATH_KEY, path.to_string_lossy().to_string());
                }

                self.app = Some(app);
            }
            Err(err) => {
                eprintln!("Failed to initialize app: {err}");
            }
        }
    }
}

pub fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt()
        .without_time()
        .with_level(true)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let viewport = egui::ViewportBuilder::default().with_icon(egui::IconData::default());
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "Photos",
        options,
        Box::new(|cc| Ok(Box::new(UiApp::new(cc)))),
    )
}
