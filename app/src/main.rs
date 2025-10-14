use eframe::egui::{self, RichText};
use egui::{ColorImage, TextureHandle, Vec2};
use std::fs;
use std::path::{Path, PathBuf};

struct Photo {
    path: PathBuf,
    thumbnail: Option<TextureHandle>, // Lazy-loaded
    loaded: bool,
}

struct PhotoLibraryApp {
    photos: Vec<Photo>,
    selected_photo: Option<usize>,
    image_dir: PathBuf,
    thumb_size: Vec2,
    columns: usize,
    first_load: bool,
}

impl PhotoLibraryApp {
    fn new() -> Self {
        let dir = dirs::picture_dir().unwrap_or_else(|| PathBuf::from("."));
        let photos = Self::scan_directory(&dir.join("pics"));

        Self {
            photos,
            selected_photo: None,
            image_dir: dir,
            thumb_size: Vec2::new(200.0, 200.0),
            columns: 1,
            first_load: true,
        }
    }

    fn scan_directory(dir: &Path) -> Vec<Photo> {
        let mut photos = Vec::new();
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if ["jpg", "jpeg", "png", "bmp"].contains(&ext.to_lowercase().as_str()) {
                        photos.push(Photo {
                            path,
                            thumbnail: None,
                            loaded: false,
                        });
                    }
                }
            }
        }
        println!("total: {}", photos.len());
        photos.sort_by_key(|p| p.path.clone());
        photos
    }

    fn load_thumbnail(&mut self, ctx: &egui::Context, index: usize) {
        if let Some(photo) = self.photos.get_mut(index) {
            if photo.loaded {
                return;
            }
            if let Ok(img) = image::open(&photo.path) {
                println!("Reading {:?}", &photo.path);
                let thumb = img.thumbnail_exact(200, 200).to_rgba8();
                let size = [thumb.width() as usize, thumb.height() as usize];
                let tex = ctx.load_texture(
                    format!("thumb-{}", photo.path.display()),
                    ColorImage::from_rgba_unmultiplied(size, thumb.as_raw()),
                    Default::default(),
                );
                photo.thumbnail = Some(tex);
                photo.loaded = true;
            }
        }
    }
}

impl eframe::App for PhotoLibraryApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(idx) = self.selected_photo {
                // Full image view
                let photo = &self.photos[idx];
                ui.vertical_centered(|ui| {
                    ui.heading(photo.path.file_name().unwrap().to_string_lossy());
                    if ui.button("← Back").clicked() {
                        self.selected_photo = None;
                    }
                    if let Ok(img) = image::open(&photo.path) {
                        let rgba = img.to_rgba8();
                        let size = [rgba.width() as usize, rgba.height() as usize];
                        let tex = ctx.load_texture(
                            format!("full-{}", photo.path.display()),
                            ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()),
                            Default::default(),
                        );
                        ui.image(&tex);
                    }
                });
                return;
            }

            // Thumbnail grid with lazy loading
            let thumb_height = self.thumb_size.y;
            let total_rows = (self.photos.len() + self.columns - 1) / self.columns;
            let total_height = total_rows as f32 * thumb_height;

            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    // Compute visible range using scroll area clip rect
                    let clip_rect = ui.clip_rect();
                    let scroll_y = ui.min_rect().top();
                    println!("scroll: {}", scroll_y);
                    let visible_start = ((clip_rect.top() - scroll_y)
                        / (thumb_height + ui.style().spacing.item_spacing.y))
                        .floor() as isize;
                    let visible_end = ((clip_rect.bottom() - scroll_y)
                        / (thumb_height + ui.style().spacing.item_spacing.y))
                        .ceil() as isize;

                    // Load thumbnails near visible region (with margin)
                    let margin = 1;
                    let start_row = (visible_start - margin).max(0) as usize;
                    let end_row = ((visible_end + margin) as usize).min(total_rows);

                    let mut start_index = start_row * self.columns;
                    let mut end_index = (end_row * self.columns).min(self.photos.len());

                    println!("{start_index} - {end_index}");
                    if start_index > end_index {
                        let x = start_index;
                        start_index = end_index;
                        end_index = x;
                    }

                    if self.first_load {
                        self.first_load = false;
                    } else {
                        for i in start_index..end_index {
                            self.load_thumbnail(ctx, i);
                        }
                    }

                    // Now draw all (using placeholders if not loaded)
                    // let mut i = self.photos.len() as i32;
                    let mut i = 0;
                    while i < self.photos.len() {
                        ui.horizontal(|ui| {
                            for _ in 0..self.columns {
                                if let Some(photo) = self.photos.get(i as usize) {
                                    let (texture, label) = if let Some(tex) = &photo.thumbnail {
                                        (Some(tex), "")
                                    } else {
                                        (None, "Loading…")
                                    };

                                    if let Some(tex) = texture {
                                        if ui
                                            .add(egui::ImageButton::new(tex).frame(false))
                                            .clicked()
                                        {
                                            self.selected_photo = Some(i as usize);
                                        }
                                    } else {
                                        ui.vertical_centered(|ui| {
                                            ui.label(RichText::new(label).line_height(Some(20f32)));
                                            ui.allocate_space(Vec2::new(
                                                200f32,
                                                180f32 - ui.style().spacing.item_spacing.y,
                                            ));
                                        });
                                    }
                                }
                                i += 1;
                            }
                        });
                        // ui.add_space(4.0);
                    }

                    // Add total height spacer so scroll behaves correctly
                    // ui.set_min_height(total_height);
                });
        });
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Photo Library (Lazy Loading)",
        options,
        Box::new(|_| Ok(Box::new(PhotoLibraryApp::new()))),
    )
}
