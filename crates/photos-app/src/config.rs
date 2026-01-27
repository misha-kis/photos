use photos_infra_cv::ImageAnalysisConfig;
use std::path::PathBuf;

pub struct Config {
    pub thumbnail_sizes: Vec<u32>,
    pub max_blocking_tasks: usize,
    pub image_analysis_config: ImageAnalysisConfig,
}

impl Default for Config {
    fn default() -> Self {
        let resources_path = resources_path();
        tracing::info!("resource path: {resources_path:?}");
        let detector_model_path = resources_path.join("assets/models/yolov12n-face.onnx");
        let embedder_model_path = resources_path.join("assets/models/facenet.onnx");
        Self {
            thumbnail_sizes: vec![128],
            max_blocking_tasks: std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4),
            image_analysis_config: ImageAnalysisConfig {
                detector_model_path,
                embedder_model_path,
                detector_image_size: 480,
                embedder_image_size: 160,
            },
        }
    }
}

fn bundle_resources_path() -> Option<PathBuf> {
    if cfg!(target_os = "macos")
        && let Ok(exe) = env::current_exe()
        && exe.to_string_lossy().contains(".app/Contents/MacOS")
    {
        Some(exe.parent().unwrap().parent().unwrap().join("Resources"))
    } else {
        None
    }
}

fn resources_path() -> PathBuf {
    match bundle_resources_path() {
        Some(path) => path,
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .into(),
    }
}
