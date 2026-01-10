use crate::components::navbar::{NavAction, show_navbar};
use crate::views::faces::FacesView;
use anyhow::Context;
use eframe::egui;
use photos_app::config::Config;
use views::gallery::GalleryView;
use views::import::ImportView;

mod app_proxy;
mod components;
mod views;

pub enum AppState {
    Gallery(GalleryView),
    Faces(FacesView),
    Import(ImportView),
}

struct UiApp {
    app_proxy: app_proxy::AppProxy,
    state: AppState,
}

impl UiApp {
    fn new() -> anyhow::Result<Self> {
        let picture_dir = dirs::picture_dir().context("Could not resolve pictures directory")?;
        let gallery_dir = picture_dir.join("picslib5");
        let config = Config::default();
        let app_proxy = app_proxy::AppProxy::new(gallery_dir, config)?;
        let state = AppState::Gallery(GalleryView::new());

        Ok(Self { app_proxy, state })
    }
}

impl eframe::App for UiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::SidePanel::left("navbar")
            .resizable(false)
            .default_width(150.0)
            .min_width(20.0)
            .show(ctx, |ui| {
                if let Some(action) = show_navbar(ui) {
                    match action {
                        NavAction::Gallery => {
                            self.state = AppState::Gallery(GalleryView::new());
                        }
                        NavAction::Faces => {
                            self.state = AppState::Faces(FacesView::new());
                        }
                        NavAction::Import => self.state = AppState::Import(ImportView::new()),
                    }
                }
            });

        egui::CentralPanel::default().show(ctx, |ui| match &mut self.state {
            AppState::Gallery(view) => view.show(ui, ctx, &mut self.app_proxy, |_| {}),
            AppState::Faces(view) => view.show(ui, ctx, &mut self.app_proxy, |_| {}),
            AppState::Import(view) => {
                let mut new_state = None;
                view.show(ui, ctx, &mut self.app_proxy, || {
                    new_state = Some(AppState::Gallery(GalleryView::new()))
                });
                if let Some(state) = new_state {
                    self.state = state;
                    self.app_proxy.refresh_images();
                }
            }
        });
    }
}

pub fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt()
        .without_time()
        .with_level(true)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    eframe::run_native(
        "Photo Library",
        eframe::NativeOptions::default(),
        Box::new(|_| Ok(Box::new(UiApp::new().unwrap()))),
    )
}
