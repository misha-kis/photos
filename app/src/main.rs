use eframe::egui;
use egui::{ColorImage, TextureHandle, Vec2};
use std::fs;
use std::path::{Path, PathBuf};

struct Photo {
    path: PathBuf,
    thumbnail: TextureHandle,
    full_image: TextureHandle,
}

struct PhotoLibraryApp {
    photos: Vec<Photo>,
    selected_photo: Option<usize>,
    image_dir: PathBuf,
}

impl PhotoLibraryApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let home = dirs::picture_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("example copy");
        let photos = Self::load_photos_from_dir(cc, &home);
        Self {
            photos,
            selected_photo: None,
            image_dir: home,
        }
    }

    fn load_photos_from_dir(cc: &eframe::CreationContext<'_>, dir: &Path) -> Vec<Photo> {
        let mut photos = Vec::new();

        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }

                if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                    if !["jpg", "jpeg", "png", "bmp", "gif"].contains(&ext.to_lowercase().as_str())
                    {
                        continue;
                    }
                }

                if let Ok(img) = image::open(&path) {
                    let thumb = img.thumbnail(200, 200).to_rgba8();
                    let full = img.thumbnail(200, 200).to_rgba8();

                    let thumb_size = [thumb.width() as usize, thumb.height() as usize];
                    let full_size = [full.width() as usize, full.height() as usize];

                    let thumb_tex = cc.egui_ctx.load_texture(
                        format!("thumb-{}", path.display()),
                        ColorImage::from_rgba_unmultiplied(thumb_size, thumb.as_raw()),
                        Default::default(),
                    );

                    let full_tex = cc.egui_ctx.load_texture(
                        format!("full-{}", path.display()),
                        ColorImage::from_rgba_unmultiplied(full_size, full.as_raw()),
                        Default::default(),
                    );

                    photos.push(Photo {
                        path,
                        thumbnail: thumb_tex,
                        full_image: full_tex,
                    });
                }
            }
        }

        photos.sort_by_key(|p| p.path.clone());
        photos
    }
}

impl eframe::App for PhotoLibraryApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(idx) = self.selected_photo {
                let photo = &self.photos[idx];
                ui.vertical_centered(|ui| {
                    ui.heading(photo.path.file_name().unwrap().to_string_lossy());
                    if ui.button("← Back to Library").clicked() {
                        self.selected_photo = None;
                    }
                    ui.add_space(10.0);
                    ui.image(&photo.full_image);
                });
            } else {
                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        let columns = 1;
                        let mut i = 0;
                        let scroll_y = ui.min_rect().top();
                        println!("{:?}", scroll_y);
                        while i < self.photos.len() {
                            ui.horizontal(|ui| {
                                for _ in 0..columns {
                                    if let Some(photo) = self.photos.get(i) {
                                        if ui
                                            .add(egui::ImageButton::new(&photo.thumbnail))
                                            .clicked()
                                        {
                                            self.selected_photo = Some(i);
                                        }
                                    }
                                    i += 1;
                                }
                            });
                            ui.add_space(4.0);
                        }
                    });
            }
        });
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Photo Library",
        options,
        Box::new(|cc| Ok(Box::new(PhotoLibraryApp::new(cc)))),
    )
}
