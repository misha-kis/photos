use eframe::egui::{ColorImage, TextureHandle, Vec2};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;

#[derive(Debug, Clone, Copy)]
pub enum ThumbSize {
    T32 = 32,
    T64 = 64,
    T128 = 128,
    T256 = 256,
}

#[derive(Debug)]
pub enum LoadRequest {
    Thumbnail {
        path: PathBuf,
        index: usize,
        thumb_size: ThumbSize,
    },
    FullImage {
        path: PathBuf,
    },
}

#[derive(Debug)]
pub(crate) enum LoadResponse {
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

pub struct Photo {
    pub path: PathBuf,
    pub thumbnail: Option<TextureHandle>, // Lazy-loaded
    pub loaded: bool,
    pub loading: bool, // Track if currently being loaded
}

pub struct PhotoLibrary {
    pub thumbnails_dir: PathBuf,
    pub thumb_size: ThumbSize,
    pub photos: Vec<Photo>,
    pub selected_photo: Option<usize>,
    pub load_tx: Sender<LoadRequest>,
    pub load_rx: Receiver<LoadResponse>,
    pub full_image_cache: Option<(PathBuf, TextureHandle)>,
}

impl PhotoLibrary {
    pub fn new(library_path: PathBuf) -> Self {
        let photos = Self::scan_directory(&library_path.join("originals"));
        // Create channels for async image loading
        let (load_tx, worker_rx) = channel::<LoadRequest>();
        let (worker_tx, load_rx) = channel::<LoadResponse>();

        // Spawn worker thread for image loading
        thread::spawn(move || {
            Self::image_loader_worker(worker_rx, worker_tx);
        });

        Self {
            thumbnails_dir: library_path.join("thumbnails"),
            thumb_size: ThumbSize::T64,
            photos,
            selected_photo: None,
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
                LoadRequest::Thumbnail {
                    path,
                    index,
                    thumb_size,
                } => {
                    if let Ok(img) = image::open(&path) {
                        println!("Loading thumbnail: {:?}", &path);
                        let thumb = img
                            .thumbnail_exact(thumb_size as u32, thumb_size as u32)
                            .to_rgba8();
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
    pub fn request_thumbnail_load(&mut self, index: usize, thumb_size: ThumbSize) {
        if let Some(photo) = self.photos.get_mut(index) {
            if !photo.loaded && !photo.loading {
                photo.loading = true;
                let _ = self.load_tx.send(LoadRequest::Thumbnail {
                    path: self
                        .thumbnails_dir
                        .join(format!("{}", self.thumb_size as u32))
                        .join(photo.path.file_name().unwrap()),
                    index,
                    thumb_size,
                });
            }
        }
    }

    // Process any loaded images from the worker thread
    pub fn process_loaded_images(&mut self, ctx: &eframe::egui::Context) {
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
