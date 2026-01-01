use photos_domain::ImageFormat;
use std::path::PathBuf;
use walkdir::WalkDir;

pub fn discover_import_items(path: PathBuf) -> Vec<PathBuf> {
    WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .map(|e| e.into_path())
        .filter_map(|p| {
            ImageFormat::try_from(p.extension()?.to_str()?)
                .ok()
                .and_then(|_| Some(p))
        })
        .collect()
}
