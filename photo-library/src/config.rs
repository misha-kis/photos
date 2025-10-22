use std::path::PathBuf;

pub struct Config {
    pub library_path: PathBuf,
    pub thumbnail_sizes: Vec<u32>,
}

impl Config {
    pub fn new(library_path: PathBuf) -> Self {
        Self {
            library_path,
            thumbnail_sizes: Vec::new(),
        }
    }

    pub fn with_thumbnail_sizes(self, thumbnail_sizes: Vec<u32>) -> Self {
        Self {
            thumbnail_sizes,
            ..self
        }
    }
}
