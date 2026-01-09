use std::cell::Cell;
use std::path::PathBuf;

use crate::app_proxy::AppProxy;
use crate::components::dynamic_grid::DynamicGrid;
use crate::components::image::image_view;
use anyhow::Context;
use eframe::egui::{self, ColorImage};
use egui_file_dialog::FileDialog;
use std::collections::HashMap;

enum ImportState {
    SelectingDirectory {
        file_dialog: FileDialog,
    },
    PreparingFileInfo(PathBuf),
    Preview {
        files_to_import: Vec<PathBuf>,
        dynamic_grid: DynamicGrid<usize, egui::TextureHandle>,
        texture_handles: HashMap<usize, egui::TextureHandle>,
        cancelled: Cell<bool>,
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
    pub fn new() -> Self {
        let mut file_dialog = FileDialog::new();
        file_dialog.pick_directory();
        Self {
            import_state: ImportState::SelectingDirectory { file_dialog },
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
            ImportState::SelectingDirectory { file_dialog } => {
                file_dialog.update(ctx);
                if let Some(path) = file_dialog.take_picked() {
                    self.import_state = ImportState::PreparingFileInfo(path.to_path_buf());
                }
            }
            ImportState::PreparingFileInfo(dir_to_import) => {
                if ui.button("Cancel").clicked() {
                    on_cancel_or_done();
                }
                app_proxy.request_discover_import_items(dir_to_import);
                app_proxy.process_events();
                if let Some(items) = app_proxy.get_discovered_items() {
                    self.import_state = ImportState::Preview {
                        files_to_import: items.clone(),
                        dynamic_grid: DynamicGrid::new(128.0),
                        texture_handles: Default::default(),
                        cancelled: Cell::new(false),
                    };
                }
            }
            ImportState::Preview {
                files_to_import,
                dynamic_grid,
                texture_handles,
                cancelled,
            } => {
                if cancelled.get() {
                    app_proxy.cancel_import_thumbnail_requests();
                    on_cancel_or_done();
                    self.import_state = ImportState::Done;
                    return;
                }

                app_proxy.process_events();

                let has_pending_thumbnails = files_to_import
                    .iter()
                    .enumerate()
                    .any(|(idx, _)| !texture_handles.contains_key(&idx));

                for (idx, path) in files_to_import.iter().enumerate() {
                    if !texture_handles.contains_key(&idx)
                        && let Some(image) = app_proxy.get_cached_import_thumbnail(path) {
                            let rgba = image.clone().into_rgba8();
                            let texture_id = format!("import-{}", idx);
                            let tex = ctx.load_texture(
                                &texture_id,
                                ColorImage::from_rgba_unmultiplied(
                                    [rgba.width() as _, rgba.height() as _],
                                    rgba.as_raw(),
                                ),
                                Default::default(),
                            );
                            texture_handles.insert(idx, tex);

                    }
                }

                if has_pending_thumbnails {
                    ctx.request_repaint();
                }

                let mut should_import = false;

                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            cancelled.set(true);
                            return;
                        }
                        if ui.button("Import").clicked() {
                            should_import = true;
                        }
                    });

                    ui.separator();
                    ui.add_space(10.0);

                    let ids: Vec<usize> =
                        files_to_import.iter().enumerate().map(|(i, _)| i).collect();

                    let get_item_data = |import_image_id: &usize| -> Option<egui::TextureHandle> {
                        if cancelled.get() {
                            return None;
                        }

                        if let Some(cached_handle) = texture_handles.get(import_image_id) {
                            return Some(cached_handle.clone());
                        }
                        let path = files_to_import
                            .get(*import_image_id)
                            .context("invalid id")
                            .ok()?;
                        app_proxy.request_import_thumbnail(path);
                        None
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

                if cancelled.get() {
                    app_proxy.cancel_import_thumbnail_requests();
                    on_cancel_or_done();
                    self.import_state = ImportState::Done;
                } else if should_import {
                    app_proxy.start_import(files_to_import.clone());
                    self.import_state = ImportState::Importing {
                        files_to_import: files_to_import.clone(),
                        dynamic_grid: DynamicGrid::new(128.0),
                        texture_handles: texture_handles.clone(),
                        done: 0,
                        total: 0,
                    };
                }
            }
            ImportState::Importing {
                files_to_import,
                dynamic_grid,
                texture_handles,
                done,
                total,
            } => {
                app_proxy.process_events();

                for (idx, path) in files_to_import.iter().enumerate() {
                    if !texture_handles.contains_key(&idx)
                        && let Some(image) = app_proxy.get_cached_import_thumbnail(path) {
                            let rgba = image.clone().into_rgba8();
                            let texture_id = format!("import-{}", idx);
                            let tex = ctx.load_texture(
                                &texture_id,
                                ColorImage::from_rgba_unmultiplied(
                                    [rgba.width() as _, rgba.height() as _],
                                    rgba.as_raw(),
                                ),
                                Default::default(),
                            );
                            texture_handles.insert(idx, tex);

                    }
                }

                let mut should_finish = false;
                if let Some(receiver) = app_proxy.get_import_workflow_receiver() {
                    while let Ok(event) = receiver.try_recv() {
                        if let Some(progress) =
                            crate::app_proxy::ImportProgress::from_app_event(&event)
                        {
                            match progress {
                                crate::app_proxy::ImportProgress::Progress(new_done, new_total) => {
                                    *done = new_done;
                                    *total = new_total;
                                }
                                crate::app_proxy::ImportProgress::Done => {
                                    should_finish = true;
                                }
                            }
                        }
                    }
                }

                if !should_finish && *done < *total {
                    ctx.request_repaint();
                }

                if should_finish {
                    self.import_state = ImportState::Done;
                    return;
                }

                let ids: Vec<usize> = files_to_import.iter().enumerate().map(|(i, _)| i).collect();
                ui.vertical(|ui| {
                    ui.label(format!("Importing: {done} / {total}"));
                    ui.separator();
                    ui.add_space(10.0);

                    let get_item_data = |import_image_id: &usize| -> Option<egui::TextureHandle> {
                        if let Some(cached_handle) = texture_handles.get(import_image_id) {
                            return Some(cached_handle.clone());
                        }
                        let path = files_to_import
                            .get(*import_image_id)
                            .context("invalid id")
                            .ok()?;
                        app_proxy.request_import_thumbnail(path);
                        None
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
            }
            ImportState::Done => {
                on_cancel_or_done();
            }
        };
    }
}
