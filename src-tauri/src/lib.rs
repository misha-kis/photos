use photos_app::config::Options;
use photos_app::App as Gallery;
use photos_app::JobEvent;
use photos_domain::ImageId;
use std::{path::PathBuf, str::FromStr};
use tauri::{Emitter, Manager};
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

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
    state: tauri::State<'_, RwLock<AppState>>,
    gallery: Option<String>,
) -> Result<(), String> {
    let gallery = Gallery::new(
        PathBuf::from(gallery.ok_or("no gallery")?),
        Options::default(),
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

#[tauri::command]
async fn get_thumbnail_path(
    state: tauri::State<'_, RwLock<AppState>>,
    image_id: String,
) -> Result<PathBuf, String> {
    tauri_plugin_log::log::info!("get_thumbnail_path: image_id={:?}", &image_id);
    let image_id = ImageId::from_str(&image_id).map_err(|e| e.to_string())?;
    let thumbnail = state
        .read()
        .await
        .gallery
        .as_ref()
        .ok_or("a gallery must be opened")?
        .get_thumbnail_path(&image_id, 128)
        .map_err(|e| e.to_string())?;
    Ok(thumbnail)
}

#[tauri::command]
async fn get_original_path(
    state: tauri::State<'_, RwLock<AppState>>,
    image_id: String,
) -> Result<PathBuf, String> {
    let image_id = ImageId::from_str(&image_id).map_err(|e| e.to_string())?;

    state
        .read()
        .await
        .gallery
        .as_ref()
        .ok_or("a gallery must be opened")?
        .get_original_path(image_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn import_images(
    app: tauri::AppHandle,
    state: tauri::State<'_, RwLock<AppState>>,
    image_paths: Vec<PathBuf>,
) -> Result<(), String> {
    let total_items = image_paths.len();
    let state = state.write().await;
    let gallery = state.gallery.as_ref().ok_or("a gallery must be opened")?;
    let mut job_handle = gallery.import_items(image_paths).await;

    while let Some(event) = job_handle.evt_rx.recv().await {
        match event {
            JobEvent::Progress(completed, total) => {
                app.emit("import-progress", (completed, total))
                    .map_err(|e| e.to_string())?;
            }
            JobEvent::Done | JobEvent::NextJob(_) => {
                app.emit("import-progress", (total_items, total_items))
                    .map_err(|e| e.to_string())?;
                return Ok(());
            }
        }
    }
    unreachable!("should only complete with JobEvent::Done or JobEvent::NextJob")
}

#[tauri::command]
async fn discover_images_for_import(
    state: tauri::State<'_, RwLock<AppState>>,
    directory: PathBuf,
) -> Result<Vec<PathBuf>, String> {
    let state = state.write().await;
    let gallery = state.gallery.as_ref().ok_or("a gallery must be opened")?;
    let cancel = CancellationToken::default();
    gallery
        .discover_import_items_async(directory, cancel)
        .await
        .map_err(|e| e.to_string())
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
        .invoke_handler(tauri::generate_handler![
            greet,
            set_gallery,
            get_image_ids,
            get_thumbnail_path,
            get_original_path,
            import_images,
            discover_images_for_import,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
