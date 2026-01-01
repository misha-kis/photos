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
                .map(|_| p)
        })
        .collect()
    // let mut res = Vec::new();
    // for path in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
    //     let p = path.into_path();
    //     let ext = p.extension();
    //     if let Some(ext) = ext {
    //         let ext = ext.to_str().unwrap();
    //         let format = ImageFormat::try_from(ext);
    //         if format.is_ok() {
    //             res.push(p);
    //         }
    //     }
    // }
    // res
}
