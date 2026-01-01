use crate::components::gallery::GalleryView;
use anyhow::Context;
use eframe::egui;
use photos_app::config::Config;

mod app_proxy;
mod components;

struct UiApp {
    app_proxy: app_proxy::AppProxy,
    gallery_view: GalleryView,
}

impl UiApp {
    fn new() -> anyhow::Result<Self> {
        let picture_dir = dirs::picture_dir().context("Could not resolve pictures directory")?;
        let gallery_dir = picture_dir.join("picslib4");
        let config = Config {
            thumbnail_sizes: vec![128],
        };
        let app_proxy = app_proxy::AppProxy::new(gallery_dir, config)?;
        let gallery_view = GalleryView::new();

        Ok(Self {
            app_proxy,
            gallery_view,
        })
    }
}

impl eframe::App for UiApp {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            self.gallery_view.show(ui, ctx, &mut self.app_proxy, |_| {});
        });
    }
}

pub fn main() -> eframe::Result<()> {
    eframe::run_native(
        "Photo Library",
        eframe::NativeOptions::default(),
        Box::new(|_| Ok(Box::new(UiApp::new().unwrap()))),
    )
}
