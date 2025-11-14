use std::path::PathBuf;

use eframe::egui::{self, ColorImage};
use image::DynamicImage;

pub struct ImportView {
    files_to_import: Vec<PathBuf>,
    previews: Vec<Option<DynamicImage>>,
}

impl ImportView {
    pub fn new() -> Self {
        Self {
            files_to_import: Vec::new(),
            previews: Vec::new(),
        }
    }

    pub fn set_files(&mut self, files: Vec<PathBuf>) {
        self.files_to_import = files;
        self.previews = vec![None; self.files_to_import.len()];
    }

    pub fn files(&self) -> &[PathBuf] {
        &self.files_to_import
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        on_cancel: impl FnOnce(),
        on_import: impl FnOnce(&[PathBuf]),
    ) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                if ui.button("Cancel").clicked() {
                    on_cancel();
                }
                if ui.button("Import").clicked() {
                    on_import(&self.files_to_import);
                }
            });

            ui.separator();
            ui.add_space(10.0);

            ui.label(format!("{} files to import", self.files_to_import.len()));
            ui.add_space(10.0);

            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    let columns = 4;
                    let mut idx = 0;
                    while idx < self.files_to_import.len() {
                        ui.horizontal(|ui| {
                            for _ in 0..columns {
                                if idx >= self.files_to_import.len() {
                                    break;
                                }
                                let file_path = &self.files_to_import[idx];
                                
                                ui.vertical(|ui| {
                                    ui.set_width(200.0);
                                    
                                    if self.previews[idx].is_none() {
                                        if let Ok(img) = image::open(file_path) {
                                            let thumb = img.thumbnail(200, 200);
                                            self.previews[idx] = Some(thumb);
                                        }
                                    }

                                    if let Some(preview) = &self.previews[idx] {
                                        let rgba = preview.to_rgba8();
                                        let size = [rgba.width() as usize, rgba.height() as usize];

                                        let tex = ctx.load_texture(
                                            format!("import-preview-{}", idx),
                                            ColorImage::from_rgba_unmultiplied(
                                                size,
                                                rgba.as_raw(),
                                            ),
                                            Default::default(),
                                        );

                                        ui.image((tex.id(), egui::Vec2::new(200.0, 200.0)));
                                    } else {
                                        let (rect, _) = ui.allocate_exact_size(
                                            egui::Vec2::new(200.0, 200.0),
                                            egui::Sense::hover(),
                                        );
                                        ui.painter().rect_filled(rect, 0.0, egui::Color32::from_gray(30));
                                        let spinner_pos = rect.center();
                                        ui.put(egui::Rect::from_center_size(spinner_pos, egui::Vec2::splat(20.0)), |ui: &mut egui::Ui| {
                                            ui.spinner()
                                        });
                                    }

                                    ui.label(
                                        file_path
                                            .file_name()
                                            .and_then(|n| n.to_str())
                                            .unwrap_or("Unknown"),
                                    );
                                });
                                idx += 1;
                            }
                        });
                    }
                });
        });
    }
}

