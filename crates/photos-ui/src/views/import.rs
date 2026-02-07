use std::path::PathBuf;

use crate::app_proxy::AppProxy;
use crate::components::dynamic_grid::DynamicGrid;
use crate::components::image::image_view;
use anyhow::Context;
use eframe::egui;
use photos_app::JobEvent;
use tokio_util::sync::CancellationToken;

enum ImportState {
    SelectingDirectory,
    PreparingFileInfo(PathBuf),
    Preview {
        files_to_import: Vec<PathBuf>,
        dynamic_grid: DynamicGrid<usize, egui::TextureHandle>,
    },
    Importing {
        files_to_import: Vec<PathBuf>,
        dynamic_grid: DynamicGrid<usize, egui::TextureHandle>,
        done: u64,
        total: u64,
    },
    Done,
}

pub struct ImportView {
    import_state: ImportState,
    cancel: CancellationToken,
}

impl ImportView {
    pub fn new() -> Self {
        Self {
            import_state: ImportState::SelectingDirectory,
            cancel: CancellationToken::new(),
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
            ImportState::SelectingDirectory => {
                ui.vertical_centered(|ui| {
                    ui.heading("Select Working Directory");
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        self.import_state = ImportState::PreparingFileInfo(path.to_path_buf());
                    }
                });
            }
            ImportState::PreparingFileInfo(dir_to_import) => {
                if ui.button("Cancel").clicked() {
                    on_cancel_or_done();
                }
                if let Some(items) =
                    app_proxy.get_discovered_import_items(dir_to_import, ctx, self.cancel.clone())
                {
                    self.import_state = ImportState::Preview {
                        files_to_import: items.clone(),
                        dynamic_grid: DynamicGrid::new(128.0),
                    };
                }
            }
            ImportState::Preview {
                files_to_import,
                dynamic_grid,
            } => {
                if self.cancel.is_cancelled() {
                    on_cancel_or_done();
                    self.import_state = ImportState::Done;
                    return;
                }

                app_proxy.process_events();

                // let has_pending_thumbnails = files_to_import
                //     .iter()
                //     .enumerate()
                //     .any(|(idx, _)| !texture_handles.contains_key(&idx));

                // if has_pending_thumbnails {
                //     ctx.request_repaint();
                // }

                let mut should_import = false;

                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            self.cancel.cancel();
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
                        let path = files_to_import
                            .get(*import_image_id)
                            .context("invalid id")
                            .ok()?;
                        app_proxy.get_import_thumbnail(path, ctx, self.cancel.clone())
                    };

                    dynamic_grid.show(
                        ui,
                        &ids,
                        get_item_data,
                        |ui, visible, size, texture_opt, click| {
                            image_view(ui, visible, size, || Ok(texture_opt.clone()), Some(click));
                        },
                        |_| {},
                        false,
                    );
                });

                if self.cancel.is_cancelled() {
                    on_cancel_or_done();
                    self.import_state = ImportState::Done;
                } else if should_import {
                    app_proxy.start_import(files_to_import.clone());
                    self.import_state = ImportState::Importing {
                        files_to_import: files_to_import.clone(),
                        dynamic_grid: DynamicGrid::new(128.0),
                        done: 0,
                        total: 0,
                    };
                }
            }
            ImportState::Importing {
                files_to_import,
                dynamic_grid,
                done,
                total,
            } => {
                app_proxy.process_events();

                let mut should_finish = false;
                if let Some(jh) = app_proxy.get_import_job_handle() {
                    while let Ok(event) = jh.evt_rx.try_recv() {
                        match event {
                            JobEvent::Progress(new_done, new_total) => {
                                *done = new_done as u64;
                                *total = new_total as u64;
                            }
                            JobEvent::Done | JobEvent::NextJob(_) => {
                                should_finish = true;
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
                        let path = files_to_import
                            .get(*import_image_id)
                            .context("invalid id")
                            .ok()?;
                        app_proxy.get_import_thumbnail(path, ctx, self.cancel.clone())
                    };

                    dynamic_grid.show(
                        ui,
                        &ids,
                        get_item_data,
                        |ui, visible, size, texture_opt, click| {
                            image_view(ui, visible, size, || Ok(texture_opt.clone()), Some(click));
                        },
                        |_| {},
                        false,
                    );
                });
            }
            ImportState::Done => {
                app_proxy.refresh_images();
                on_cancel_or_done();
            }
        };
    }
}

impl Drop for ImportView {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}
