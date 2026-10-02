use photos_app::config::Options;
use photos_app::App as Gallery;
use photos_app::JobEvent;
use photos_domain::{ImageId, Uuid};
use serde::Serialize;
use std::path::PathBuf;
use tauri::{Emitter, Manager};
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

#[derive(Default)]
struct AppState {
    gallery: Option<Gallery>,
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
async fn get_image_ids_with_paths(
    state: tauri::State<'_, RwLock<AppState>>,
) -> Result<Vec<(ImageId, PathBuf, PathBuf)>, String> {
    let ids_with_paths: Vec<_> = state
        .read()
        .await
        .gallery
        .as_ref()
        .ok_or("a gallery must be opened")?
        .get_image_ids_with_paths_async()
        .await
        .map_err(|e| e.to_string())?;
    Ok(ids_with_paths)
}

#[derive(Serialize)]
struct PersonCluster {
    id: Uuid,
    thumbnail_path: PathBuf,
    photo_count: usize,
    detection_ids: Vec<Uuid>,
}

#[tauri::command]
async fn get_people(
    state: tauri::State<'_, RwLock<AppState>>,
) -> Result<Vec<PersonCluster>, String> {
    let state = state.read().await;
    let gallery = state.gallery.as_ref().ok_or("a gallery must be opened")?;
    let clusters = gallery
        .get_face_clusters_async()
        .await
        .map_err(|e| e.to_string())?;
    let mut people = Vec::with_capacity(clusters.len());

    for (id, detection_ids) in clusters {
        let thumbnail_path = gallery.get_face_thumbnail_path(id);
        let records = gallery
            .get_image_records_for_face_cluster_async(&detection_ids)
            .await
            .map_err(|e| e.to_string())?;
        people.push(PersonCluster {
            id,
            thumbnail_path,
            photo_count: records.len(),
            detection_ids,
        });
    }

    people.sort_by(|a, b| {
        b.photo_count
            .cmp(&a.photo_count)
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok(people)
}

#[tauri::command]
async fn get_person_photos(
    state: tauri::State<'_, RwLock<AppState>>,
    detection_ids: Vec<Uuid>,
) -> Result<Vec<(ImageId, PathBuf, PathBuf)>, String> {
    let state = state.read().await;
    state
        .gallery
        .as_ref()
        .ok_or("a gallery must be opened")?
        .get_image_records_for_face_cluster_async(&detection_ids)
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
                .level(tauri_plugin_log::log::LevelFilter::Warn)
                .build(),
        )
        .setup(|app| {
            app.manage(RwLock::new(AppState::default()));
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            set_gallery,
            import_images,
            discover_images_for_import,
            get_image_ids_with_paths,
            get_people,
            get_person_photos,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
