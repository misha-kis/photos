use base64::{engine::general_purpose, Engine as _};
use futures::lock::Mutex;
use image::{ImageFormat, ImageReader};
use std::path::{Path, PathBuf};
use tauri::{Manager, State};

struct AppState {
    names: Vec<String>,
    originals_dir: PathBuf,
    thumbnails_dir: PathBuf,
}

impl AppState {
    fn new(gallery_dir: &Path) -> Self {
        let originals_dir = gallery_dir.join("originals");
        let thumbnails_dir = gallery_dir.join("thumbnails").join("128");
        let mut names = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&thumbnails_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.is_file() {
                    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
                    if matches!(
                        ext.to_lowercase().as_str(),
                        "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp"
                    ) {
                        names.push(path.file_name().unwrap().to_string_lossy().into_owned());
                    }
                }
            }
        }
        names.sort();
        AppState {
            names,
            originals_dir,
            thumbnails_dir,
        }
    }

    fn get_thumbnail_path(&self, index: usize) -> Option<PathBuf> {
        self.names
            .get(index)
            .map(|name| self.thumbnails_dir.join(name))
    }

    fn get_original_path(&self, index: usize) -> Option<PathBuf> {
        self.names
            .get(index)
            .map(|name| self.originals_dir.join(name))
    }

    fn len(&self) -> usize {
        self.names.len()
    }
}

#[tauri::command]
async fn get_total_image_count(state: State<'_, Mutex<AppState>>) -> Result<usize, String> {
    println!("get_total_image_count");
    let app_state = state.lock().await;
    Ok(app_state.len())
}

#[tauri::command]
async fn load_thumbnail(
    index: usize,
    state: State<'_, Mutex<AppState>>,
) -> Result<String, tauri::Error> {
    println!("waiting for lock for index {}", index);
    let app_state = state.lock().await;
    println!("load_thumbnail {}", index);

    let thumbnail_path = app_state
        .get_thumbnail_path(index)
        .ok_or_else(|| tauri::Error::from(anyhow::anyhow!("Index out of bounds")))?;

    drop(app_state);
    let thumbnail_size = 200; // e.g., 200x200 pixels

    tauri::async_runtime::spawn_blocking(move || {
        let mut buffer = Vec::new();

        let img = ImageReader::open(thumbnail_path).unwrap().decode().unwrap();
        let resized_img = img.resize(
            thumbnail_size,
            thumbnail_size,
            image::imageops::FilterType::Lanczos3,
        );

        resized_img
            .write_to(&mut std::io::Cursor::new(&mut buffer), ImageFormat::WebP)
            .unwrap();

        println!("Sending thumbnail");
        general_purpose::STANDARD.encode(&buffer)
    })
    .await
}

#[tauri::command]
async fn load_image(
    index: usize,
    state: State<'_, Mutex<AppState>>,
) -> Result<String, tauri::Error> {
    println!("waiting for lock for index {}", index);
    let app_state = state.lock().await;
    println!("load_image {}", index);

    let image_path = app_state
        .get_original_path(index)
        .ok_or_else(|| tauri::Error::from(anyhow::anyhow!("Index out of bounds")))?;

    drop(app_state);

    tauri::async_runtime::spawn_blocking(move || {
        let mut buffer = Vec::new();
        ImageReader::open(image_path)
            .unwrap()
            .decode()
            .unwrap()
            .write_to(&mut std::io::Cursor::new(&mut buffer), ImageFormat::WebP)
            .unwrap();

        println!("Sending thumbnail");
        general_purpose::STANDARD.encode(&buffer)
    })
    .await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let gallery_dir = PathBuf::from("/Users/mikhailkiselyov/Pictures/picslib");
            std::fs::create_dir_all(&gallery_dir)
                .unwrap_or_else(|e| eprintln!("Failed to create gallery dir: {}", e));
            if std::fs::read_dir(&gallery_dir).map_or(true, |mut i| i.next().is_none()) {
                println!("Gallery directory is empty. Creating dummy images...");
                let dummy_image_path = gallery_dir.join("dummy_image.png");
                if let Err(e) = image::RgbImage::new(30, 30).save(&dummy_image_path) {
                    eprintln!("Failed to create dummy image: {}", e);
                } else {
                    println!("Created {}", dummy_image_path.display());
                }
                for i in 0..50 {
                    let path = gallery_dir.join(format!("dummy_image_{}.png", i));
                    let img = image::RgbImage::new(30, 30);
                    if let Err(e) = img.save(&path) {
                        eprintln!("Failed to create dummy image {}: {}", i, e);
                    }
                }
                println!("Finished creating dummy images.");
            }
            let initial_state = AppState::new(&gallery_dir);
            app.manage(Mutex::new(initial_state));
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_total_image_count,
            load_thumbnail,
            load_image,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
