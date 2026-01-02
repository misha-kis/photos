use crate::AppState;
use std::path::PathBuf;

use crate::app_proxy::AppProxy;
use crate::components::dynamic_grid::DynamicGrid;
use crate::components::image::image_view;
use anyhow::Context;
use eframe::egui::{self, ColorImage};
use image::DynamicImage;
use std::collections::HashMap;

enum ImportState {
    SelectingDirectory,
    PreparingFileInfo(PathBuf),
    Preview {
        files_to_import: Vec<PathBuf>,
        dynamic_grid: DynamicGrid<usize, egui::TextureHandle>,
        texture_handles: HashMap<usize, egui::TextureHandle>,
    },
    Importing {
        files_to_import: Vec<PathBuf>,
        dynamic_grid: DynamicGrid<usize, egui::TextureHandle>,
        texture_handles: HashMap<usize, egui::TextureHandle>,
        done: u64,
        total: u64,
    },
    Done,
}

pub struct ImportView {
    import_state: ImportState,
}

impl ImportView {
    pub fn new(dir_to_import: PathBuf) -> Self {
        Self {
            import_state: ImportState::PreparingFileInfo(dir_to_import),
        }
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        app_proxy: &mut AppProxy,
        on_cancel_or_done: impl FnOnce(),
    ) {
        match &mut self.import_state {
            ImportState::SelectingDirectory => todo!(),
            ImportState::PreparingFileInfo(dir_to_import) => {
                if ui.button("Cancel").clicked() {
                    on_cancel_or_done();
                }
                if let Ok(Some(items)) = app_proxy.try_discover_import_items(&dir_to_import) {
                    let n_items = items.len();
                    self.import_state = ImportState::Preview {
                        files_to_import: items,
                        dynamic_grid: DynamicGrid::new(128.0),
                        texture_handles: Default::default(),
                    };
                }
            }
            ImportState::Preview {
                files_to_import,
                dynamic_grid,
                texture_handles,
            } => {
                let ids: Vec<usize> = files_to_import.iter().enumerate().map(|(i, _)| i).collect();
                let mut new_state = None;
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            on_cancel_or_done();
                            new_state = Some(ImportState::Done);
                        }
                        if ui.button("Import").clicked() {
                            app_proxy.start_import(files_to_import.clone());
                            new_state = Some(ImportState::Importing {
                                files_to_import: files_to_import.clone(),
                                dynamic_grid: DynamicGrid::new(128.0),
                                texture_handles: texture_handles.clone(),
                                done: 0,
                                total: 0,
                            });
                        }
                    });

                    ui.separator();
                    ui.add_space(10.0);

                    let get_item_data = |import_image_id: &usize| -> Option<egui::TextureHandle> {
                        if let Some(cached_handle) = texture_handles.get(&import_image_id) {
                            return Some(cached_handle.clone());
                        }
                        let path = files_to_import
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
                                texture_handles.insert(*import_image_id, tex.clone());
                                Some(tex)
                            }
                            Ok(None) => None,
                            Err(e) => {
                                tracing::error!("could not fetch item: {e:?}");
                                None
                            }
                        }
                    };

                    dynamic_grid.show(
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
                if let Some(new_state) = new_state {
                    self.import_state = new_state;
                }
            }
            ImportState::Importing {
                files_to_import,
                dynamic_grid,
                texture_handles,
                done,
                total,
            } => {
                let ids: Vec<usize> = files_to_import.iter().enumerate().map(|(i, _)| i).collect();
                ui.vertical(|ui| {
                    ui.label(format!("Importing: {done} / {total}"));
                    ui.separator();
                    ui.add_space(10.0);

                    let get_item_data = |import_image_id: &usize| -> Option<egui::TextureHandle> {
                        if let Some(cached_handle) = texture_handles.get(&import_image_id) {
                            return Some(cached_handle.clone());
                        }
                        let path = files_to_import
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
                                texture_handles.insert(*import_image_id, tex.clone());
                                Some(tex)
                            }
                            Ok(None) => None,
                            Err(e) => {
                                tracing::error!("could not fetch item: {e:?}");
                                None
                            }
                        }
                    };

                    dynamic_grid.show(
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
                match app_proxy.check_import_progress() {
                    crate::app_proxy::ImportProgress::None => {}
                    crate::app_proxy::ImportProgress::Progress(new_done, new_total) => {
                        *done = new_done;
                        *total = new_total
                    }
                    crate::app_proxy::ImportProgress::Done => {
                        self.import_state = ImportState::Done;
                    }
                }
            }
            ImportState::Done => {
                on_cancel_or_done();
            }
        };
    }
}
