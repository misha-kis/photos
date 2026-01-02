use crate::AppState;
use std::path::PathBuf;

use crate::app_proxy::AppProxy;
use crate::components::dynamic_grid::DynamicGrid;
use crate::components::image::image_view;
use anyhow::Context;
use eframe::egui::{self, ColorImage};
use image::DynamicImage;
use std::collections::HashMap;

struct ImportData {
    files_to_import: Vec<PathBuf>,
    previews: Vec<Option<egui::TextureHandle>>,
    dynamic_grid: DynamicGrid<usize, egui::TextureHandle>,
    texture_handles: HashMap<usize, egui::TextureHandle>,
}

pub struct ImportView {
    dir_to_import: PathBuf,
    import_data: Option<ImportData>,
}

impl ImportView {
    pub fn new(dir_to_import: PathBuf) -> Self {
        Self {
            dir_to_import,
            import_data: None,
        }
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        app_proxy: &mut AppProxy,
        on_cancel: impl FnOnce(),
    ) {
        if let Some(import_data) = &mut self.import_data {
            let ids: Vec<usize> = import_data
                .files_to_import
                .iter()
                .enumerate()
                .map(|(i, _)| i)
                .collect();

            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        on_cancel();
                    }
                    if ui.button("Import").clicked() {
                        app_proxy.start_import(import_data.files_to_import.clone());
                    }
                });

                ui.separator();
                ui.add_space(10.0);

                let get_item_data = |import_image_id: &usize| -> Option<egui::TextureHandle> {
                    if let Some(cached_handle) = import_data.texture_handles.get(&import_image_id) {
                        return Some(cached_handle.clone());
                    }
                    let path = import_data
                        .files_to_import
                        .get(*import_image_id)
                        .context("invalid id")
                        .ok()?;
                    match app_proxy.try_render_import_thumbnail(&path) {
                        Ok(Some(image)) => {
                            let rgba = image.into_rgba8();
                            let texture_id = format!("import-{}", import_image_id);
                            let tex = ctx.load_texture(
                                &texture_id,
                                ColorImage::from_rgba_unmultiplied(
                                    [rgba.width() as _, rgba.height() as _],
                                    rgba.as_raw(),
                                ),
                                Default::default(),
                            );
                            import_data
                                .texture_handles
                                .insert(*import_image_id, tex.clone());
                            Some(tex)
                        }
                        Ok(None) => None,
                        Err(e) => {
                            tracing::error!("could not fetch item: {e:?}");
                            None
                        }
                    }
                };

                import_data.dynamic_grid.show(
                    ui,
                    ctx,
                    &ids,
                    get_item_data,
                    |ui, visible, size, texture_opt, click| {
                        image_view(ui, visible, size, || Ok(texture_opt.clone()), Some(click));
                    },
                    |_| {},
                );
            });
        } else {
            if ui.button("Cancel").clicked() {
                on_cancel();
            }
            if let Ok(Some(items)) = app_proxy.try_discover_import_items(&self.dir_to_import) {
                let n_items = items.len();
                self.import_data = Some(ImportData {
                    files_to_import: items,
                    previews: vec![None; n_items],
                    dynamic_grid: DynamicGrid::new(128.0),
                    texture_handles: Default::default(),
                })
            }
        }
    }
}
