use eframe::egui;
use egui::{ColorImage, TextureHandle, Vec2};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

#[derive(Debug)]
enum LoadRequest {
    Thumbnail { path: PathBuf, index: usize },
    FullImage { path: PathBuf },
}

#[derive(Debug)]
enum LoadResponse {
    Thumbnail {
        index: usize,
        path: PathBuf,
        image_data: Vec<u8>,
        width: u32,
        height: u32,
    },
    FullImage {
        path: PathBuf,
        image_data: Vec<u8>,
        width: u32,
        height: u32,
    },
}

struct Photo {
    path: PathBuf,
    thumbnail: Option<TextureHandle>, // Lazy-loaded
    loaded: bool,
    loading: bool, // Track if currently being loaded
}

struct PhotoLibraryApp {
    photos: Vec<Photo>,
    selected_photo: Option<usize>,
    _image_dir: PathBuf,
    thumb_size: Vec2,
    columns: usize,
    first_load: bool,
    load_tx: Sender<LoadRequest>,
    load_rx: Receiver<LoadResponse>,
    full_image_cache: Option<(PathBuf, TextureHandle)>,
}

impl PhotoLibraryApp {
    fn new() -> Self {
        let dir = dirs::picture_dir().unwrap_or_else(|| PathBuf::from("."));
        let photos = Self::scan_directory(&dir.join("pics"));

        // Create channels for async image loading
        let (load_tx, worker_rx) = channel::<LoadRequest>();
        let (worker_tx, load_rx) = channel::<LoadResponse>();

        // Spawn worker thread for image loading
        thread::spawn(move || {
            Self::image_loader_worker(worker_rx, worker_tx);
        });

        Self {
            photos,
            selected_photo: None,
            _image_dir: dir,
            thumb_size: Vec2::new(200.0, 200.0),
            columns: 2,
            first_load: true,
            load_tx,
            load_rx,
            full_image_cache: None,
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
                            loading: false,
                        });
                    }
                }
            }
        }
        println!("total: {}", photos.len());
        photos.sort_by_key(|p| p.path.clone());
        photos
    }

    // Worker thread that loads images in background
    fn image_loader_worker(rx: Receiver<LoadRequest>, tx: Sender<LoadResponse>) {
        while let Ok(request) = rx.recv() {
            match request {
                LoadRequest::Thumbnail { path, index } => {
                    if let Ok(img) = image::open(&path) {
                        println!("Loading thumbnail: {:?}", &path);
                        let thumb = img.thumbnail_exact(200, 200).to_rgba8();
                        let width = thumb.width();
                        let height = thumb.height();
                        let image_data = thumb.into_raw();
                        
                        let _ = tx.send(LoadResponse::Thumbnail {
                            index,
                            path,
                            image_data,
                            width,
                            height,
                        });
                    }
                }
                LoadRequest::FullImage { path } => {
                    if let Ok(img) = image::open(&path) {
                        println!("Loading full image: {:?}", &path);
                        let rgba = img.to_rgba8();
                        let width = rgba.width();
                        let height = rgba.height();
                        let image_data = rgba.into_raw();
                        
                        let _ = tx.send(LoadResponse::FullImage {
                            path,
                            image_data,
                            width,
                            height,
                        });
                    }
                }
            }
        }
    }

    // Request thumbnail loading for a specific index
    fn request_thumbnail_load(&mut self, index: usize) {
        if let Some(photo) = self.photos.get_mut(index) {
            if !photo.loaded && !photo.loading {
                photo.loading = true;
                let _ = self.load_tx.send(LoadRequest::Thumbnail {
                    path: photo.path.clone(),
                    index,
                });
            }
        }
    }

    // Process any loaded images from the worker thread
    fn process_loaded_images(&mut self, ctx: &egui::Context) {
        // Process all available responses (non-blocking)
        while let Ok(response) = self.load_rx.try_recv() {
            match response {
                LoadResponse::Thumbnail {
                    index,
                    path,
                    image_data,
                    width,
                    height,
                } => {
                    if let Some(photo) = self.photos.get_mut(index) {
                        let size = [width as usize, height as usize];
                        let tex = ctx.load_texture(
                            format!("thumb-{}", path.display()),
                            ColorImage::from_rgba_unmultiplied(size, &image_data),
                            Default::default(),
                        );
                        photo.thumbnail = Some(tex);
                        photo.loaded = true;
                        photo.loading = false;
                    }
                    // Request repaint to show the newly loaded thumbnail
                    ctx.request_repaint();
                }
                LoadResponse::FullImage {
                    path,
                    image_data,
                    width,
                    height,
                } => {
                    let size = [width as usize, height as usize];
                    let tex = ctx.load_texture(
                        format!("full-{}", path.display()),
                        ColorImage::from_rgba_unmultiplied(size, &image_data),
                        Default::default(),
                    );
                    self.full_image_cache = Some((path, tex));
                    // Request repaint to show the newly loaded full image
                    ctx.request_repaint();
                }
            }
        }
    }
}

impl eframe::App for PhotoLibraryApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Process any loaded images from the worker thread
        self.process_loaded_images(ctx);

        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(idx) = self.selected_photo {
                // Full image view
                let photo = &self.photos[idx];
                ui.vertical_centered(|ui| {
                    ui.heading(photo.path.file_name().unwrap().to_string_lossy());
                    if ui.button("← Back").clicked() {
                        self.selected_photo = None;
                        self.full_image_cache = None;
                    }
                    
                    // Check if we have the full image cached
                    if let Some((cached_path, tex)) = &self.full_image_cache {
                        if cached_path == &photo.path {
                            ui.image(tex);
                        }
                    } else {
                        // Request loading if not cached
                        let _ = self.load_tx.send(LoadRequest::FullImage {
                            path: photo.path.clone(),
                        });
                        ui.spinner();
                        ui.label("Loading...");
                    }
                });
                return;
            }

            // Thumbnail grid with lazy loading
            let thumb_height = self.thumb_size.y;
            let total_rows = (self.photos.len() + self.columns - 1) / self.columns;

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
                            self.request_thumbnail_load(i);
                        }
                    }

                    // Now draw all (using placeholders if not loaded)
                    // let mut i = self.photos.len() as i32;
                    let mut i = 0;
                    while i < self.photos.len() {
                        ui.horizontal(|ui| {
                            for _ in 0..self.columns {
                                if let Some(photo) = self.photos.get(i as usize) {
                                    if let Some(tex) = &photo.thumbnail {
                                        if ui
                                            .add(egui::ImageButton::new(tex).frame(false))
                                            .clicked()
                                        {
                                            self.selected_photo = Some(i as usize);
                                        }
                                    } else {
                                        ui.allocate_space(Vec2::new(200f32, 200f32));
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
