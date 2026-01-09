use photos_infra_cv::ImageAnalysisConfig;
use std::path::PathBuf;

pub struct Config {
    pub thumbnail_sizes: Vec<u32>,
    pub max_blocking_tasks: usize,
    pub image_analysis_config: ImageAnalysisConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            thumbnail_sizes: vec![128],
            max_blocking_tasks: std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4),
            image_analysis_config: ImageAnalysisConfig {
                detector_model_path: PathBuf::from(
                    "/Users/mikhailkiselyov/code/misc/photos/models/yolov12n-face.onnx",
                ),
                embedder_model_path: PathBuf::from(
                    "/Users/mikhailkiselyov/code/misc/photos/models/facenet.onnx",
                ),
                detector_image_size: 480,
                embedder_image_size: 160,
            },
        }
    }
}
