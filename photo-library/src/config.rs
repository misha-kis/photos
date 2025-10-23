use std::path::PathBuf;

use crate::workers::cv_worker::CvConfig;

pub struct Config {
    pub library_path: PathBuf,
    pub thumbnail_sizes: Vec<u32>,
    pub cv_config: CvConfig,
}

impl Config {
    pub fn new(library_path: PathBuf, cv_config: CvConfig) -> Self {
        Self {
            library_path,
            thumbnail_sizes: Vec::new(),
            cv_config,
        }
    }

    pub fn with_thumbnail_sizes(self, thumbnail_sizes: Vec<u32>) -> Self {
        Self {
            thumbnail_sizes,
            ..self
        }
    }
}
