use std::{fs, path::PathBuf};

async fn download(model: &str) {
    let url = format!("https://github.com/misha-kis/photos/releases/download/v0.0.0/{model}");
    let out_dir = PathBuf::from("../../assets/models");
    fs::create_dir_all(&out_dir).unwrap();
    let model_path = out_dir.join(model);
    let bytes = reqwest::get(url).await.unwrap().bytes().await.unwrap();
    tokio::fs::write(model_path, bytes).await.unwrap();
}

#[tokio::main]
async fn main() {
    tokio::join!(
        download("facenet_240.onnx"),
        download("yolov12n-face_640.onnx")
    );
    println!("cargo:rerun-if-changed=build.rs");
}
