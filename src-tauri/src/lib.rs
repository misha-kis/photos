use base64::{engine::general_purpose, Engine as _};
use futures::lock::Mutex;
use image::{ImageFormat, ImageReader};
use std::path::{Path, PathBuf};
use tauri::{Manager, State};

struct AppState {
    image_paths: Vec<PathBuf>,
}

impl AppState {
    fn new(gallery_dir: &Path) -> Self {
        let mut paths = Vec::new();
        if let Ok(entries) = std::fs::read_dir(gallery_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.is_file() {
                    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
                    if matches!(
                        ext.to_lowercase().as_str(),
                        "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp"
                    ) {
                        paths.push(path);
                    }
                }
            }
        }
        paths.sort();
        AppState { image_paths: paths }
    }
}

#[tauri::command]
async fn get_total_image_count(state: State<'_, Mutex<AppState>>) -> Result<usize, String> {
    println!("get_total_image_count");
    let app_state = state.lock().await;
    Ok(app_state.image_paths.len())
}

#[tauri::command]
async fn load_thumbnail(
    index: usize,
    state: State<'_, Mutex<AppState>>,
) -> Result<String, tauri::Error> {
    println!("waiting for lock for index {}", index);
    let app_state = state.lock().await;
    println!("load_thumbnail {}", index);

    if index >= app_state.image_paths.len() {
        return Err(tauri::Error::from(anyhow::anyhow!("Index out of bounds")));
    }

    let image_path = app_state.image_paths[index].clone();
    drop(app_state);
    let thumbnail_size = 200; // e.g., 200x200 pixels

    tauri::async_runtime::spawn_blocking(move || {
        let img = ImageReader::open(image_path).unwrap().decode().unwrap();

        let resized_img = img.resize(
            thumbnail_size,
            thumbnail_size,
            image::imageops::FilterType::Lanczos3,
        );

        let mut buffer = Vec::new();
        resized_img
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
            let gallery_dir =
                PathBuf::from("/Users/mikhailkiselyov/Pictures/picslib/thumbnails/128");
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
            load_thumbnail
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
