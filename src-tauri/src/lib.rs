use base64::{engine::general_purpose, Engine as _};
use futures::lock::Mutex;
use image::{DynamicImage, ImageFormat, ImageReader};
use photo_library::{Config, CvConfig, PhotoLibrary};
use std::path::{Path, PathBuf};
use tauri::{Manager, State};
use time::macros::{format_description, offset};
use tracing_subscriber::fmt::time::OffsetTime;

struct AppState {
    library: PhotoLibrary,
    rt: tokio::runtime::Runtime,
}

impl AppState {
    fn new(gallery_dir: PathBuf) -> Self {
        let workspace_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf();
        let cv_cfg = CvConfig {
            face_detector_model_path: workspace_path.join("models").join("yolov12n-face.onnx"),
            face_embedder_model_path: workspace_path.join("models").join("facenet.onnx"),
            face_detector_image_size: 480,
            face_embedder_image_size: 160,
        };
        let cfg = Config::new(gallery_dir, cv_cfg);
        println!("Starting runtime");
        let rt = tokio::runtime::Runtime::new().unwrap();
        let library = rt.block_on(PhotoLibrary::new(cfg)).unwrap();

        AppState { library, rt }
    }

    async fn load_thumbnail(&mut self, id: u32) -> Result<DynamicImage, tauri::Error> {
        self.library
            .get_thumbnail(id + 1)
            .await
            .map_err(|e| tauri::Error::from(e))
    }

    async fn load_full_image(&mut self, id: u32) -> Result<DynamicImage, tauri::Error> {
        self.library
            .get_full_image(id + 1)
            .await
            .map_err(|e| tauri::Error::from(e))
    }

    async fn len(&self) -> usize {
        self.library.get_number_of_images().await.unwrap()
    }
}

#[tauri::command]
async fn get_total_image_count(state: State<'_, Mutex<AppState>>) -> Result<usize, String> {
    println!("get_total_image_count");
    let app_state = state.lock().await;
    Ok(app_state.len().await)
}

#[tauri::command]
async fn load_thumbnail(
    index: u32,
    state: State<'_, Mutex<AppState>>,
) -> Result<String, tauri::Error> {
    println!("waiting for lock for index {}", index);
    let mut app_state = state.lock().await;
    println!("load_thumbnail {}", index);

    let img = app_state.load_thumbnail(index).await?;

    drop(app_state);
    let thumbnail_size = 200; // e.g., 200x200 pixels

    tauri::async_runtime::spawn_blocking(move || {
        let mut buffer = Vec::new();

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
async fn load_image(index: u32, state: State<'_, Mutex<AppState>>) -> Result<String, tauri::Error> {
    println!("waiting for lock for index {}", index);
    let mut app_state = state.lock().await;
    println!("load_image {}", index);

    let img = app_state.load_full_image(index).await?;

    drop(app_state);

    tauri::async_runtime::spawn_blocking(move || {
        let mut buffer = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buffer), ImageFormat::WebP)
            .unwrap();

        println!("Sending thumbnail");
        general_purpose::STANDARD.encode(&buffer)
    })
    .await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_level(true)
        .with_thread_names(true)
        .with_thread_ids(true)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    tracing::info!("Starting application");
    tauri::Builder::default()
        .setup(|app| {
            let gallery_dir = PathBuf::from("/Users/mikhailkiselyov/Pictures/picslib2");
            let mut state = AppState::new(gallery_dir);

            let to_import_path = PathBuf::from("/Users/mikhailkiselyov/Pictures/pics2");
            // PathBuf::from("/Users/mikhailkiselyov/code/misc/photos/test_data/example.jpeg");

            println!("Starting runtime");
            state
                .rt
                .block_on(state.library.import_photo(to_import_path))
                .unwrap();
            println!("Photo imported");

            app.manage(Mutex::new(state));
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
