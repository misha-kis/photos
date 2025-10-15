// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

use base64::{engine::general_purpose, Engine as _};
use image::{GenericImageView, ImageFormat, ImageReader};
use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};
use tauri::{Manager, State};

// This struct will hold our application state, like image paths
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
                    // Basic check for common image extensions
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
        // Sort paths for a consistent order
        paths.sort();
        AppState { image_paths: paths }
    }
}

// Command to get the total number of images
#[tauri::command]
fn get_total_image_count(state: State<Mutex<AppState>>) -> Result<usize, String> {
    println!("get_total_image_count");
    let app_state = state.lock().unwrap();
    Ok(app_state.image_paths.len())
}

// #[tauri::command]
// fn get_total_image_count() -> Result<usize, String> {
//     println!("get_total_image_count");
//     Ok(1)
// }

// Command to load a thumbnail for a given index
// Returns a base64 encoded string of the thumbnail
#[tauri::command]
fn load_thumbnail(index: usize, state: State<Mutex<AppState>>) -> Result<String, String> {
    let app_state = state.lock().unwrap();
    println!("load_thumbnail {}", index);

    if index >= app_state.image_paths.len() {
        return Err("Index out of bounds".into());
    }

    let image_path = &app_state.image_paths[index];
    let thumbnail_size = 200; // e.g., 200x200 pixels

    let img = ImageReader::open(image_path)
        .map_err(|e| format!("Failed to open image {}: {}", image_path.display(), e))?
        .decode()
        .map_err(|e| format!("Failed to decode image {}: {}", image_path.display(), e))?;

    let resized_img = img.resize(
        thumbnail_size,
        thumbnail_size,
        image::imageops::FilterType::Lanczos3,
    );

    let mut buffer = Vec::new();
    resized_img
        .write_to(&mut std::io::Cursor::new(&mut buffer), ImageFormat::Png)
        .map_err(|e| format!("Failed to write thumbnail to buffer: {}", e))?;

    Ok(general_purpose::STANDARD.encode(&buffer))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // let app_dir = app
            //     .path_resolver()
            //     .app_local_data_dir()
            //     .unwrap_or_else(|| app.path_resolver().app_data_dir().unwrap());
            // let gallery_dir = app_dir.join("gallery");
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
                // Create multiple dummy images for better testing of virtual scrolling
                for i in 0..50 {
                    // Create 500 dummy images
                    let path = gallery_dir.join(format!("dummy_image_{}.png", i));
                    let img = image::RgbImage::new(30, 30); // Simple blank image
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
