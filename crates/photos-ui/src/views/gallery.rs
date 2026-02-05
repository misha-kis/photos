use crate::app_proxy::AppProxy;
use crate::components::dynamic_grid::DynamicGrid;
use crate::components::image::image_view;
use eframe::egui;
use photos_domain::ImageId;

pub struct GalleryView {
    dynamic_grid: DynamicGrid<ImageId, egui::TextureHandle>,
}

impl GalleryView {
    pub fn new() -> Self {
        Self {
            dynamic_grid: DynamicGrid::new(128.0),
        }
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        app_proxy: &mut AppProxy,
        on_item_clicked: impl FnMut(usize),
    ) {
        app_proxy.process_events();

        let image_ids = app_proxy.image_ids.clone();
        let get_item_data = |image_id: &ImageId| -> Option<egui::TextureHandle> {
            app_proxy.get_thumbnail(*image_id, ctx)
        };

        self.dynamic_grid.show(
            ui,
            &image_ids,
            get_item_data,
            |ui, visible, size, texture_opt, click| {
                image_view(ui, visible, size, || Ok(texture_opt.clone()), Some(click));
            },
            on_item_clicked,
            true,
        );
    }
}
