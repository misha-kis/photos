use photos_app::config::Options;
use photos_app::App as Gallery;
use photos_domain::ImageId;
use std::path::PathBuf;
use tauri::Manager;
use tauri_plugin_log::{Target, TargetKind};
use tokio::sync::RwLock;

#[derive(Default)]
struct AppState {
    gallery: Option<Gallery>,
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn set_gallery(
    // app: tauri::AppHandle,
    state: tauri::State<'_, RwLock<AppState>>,
    gallery: Option<String>,
) -> Result<(), String> {
    let runtime_handle = tokio::runtime::Handle::current();
    let gallery = Gallery::new(
        PathBuf::from(gallery.ok_or("no gallery")?),
        Options::default(),
        runtime_handle,
    )
    .await
    .map_err(|e| e.to_string())?;
    let mut s = state.write().await;
    s.gallery = Some(gallery);
    Ok(())
}

#[tauri::command]
async fn get_image_ids(state: tauri::State<'_, RwLock<AppState>>) -> Result<Vec<ImageId>, String> {
    let ids: Vec<_> = state
        .read()
        .await
        .gallery
        .as_ref()
        .ok_or("a gallery must be opened")?
        .get_image_ids_async()
        .await
        .map_err(|e| e.to_string())?;
    // .iter()
    // .map(|id| id.to_string())
    // .collect();
    tauri_plugin_log::log::info!("get_image_ids: {:?}", &ids);
    Ok(ids)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(tauri_plugin_log::log::LevelFilter::Info)
                .build(),
        )
        .setup(|app| {
            app.manage(RwLock::new(AppState::default()));
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_log::Builder::new().build())
        .invoke_handler(tauri::generate_handler![greet, set_gallery, get_image_ids])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
