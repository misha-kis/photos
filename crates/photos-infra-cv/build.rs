use std::{fs, path::PathBuf};

const MODEL_NAME: &'static str = "facenet_112.onnx";
const GITHUB_RELEASE_URL: &'static str = "https://github.com/misha-kis/photos/releases/download/";
const RELEASE: &'static str = "v0.0.1";
const OUT_DIR: &'static str = "../../assets/models";

#[tokio::main]
async fn main() {
    let url = GITHUB_RELEASE_URL.to_owned() + RELEASE + "/" + MODEL_NAME;
    let out_dir = PathBuf::from(OUT_DIR);
    fs::create_dir_all(&out_dir).unwrap();
    let model_path = out_dir.join(MODEL_NAME);
    let bytes = reqwest::get(url).await.unwrap().bytes().await.unwrap();
    tokio::fs::write(model_path, bytes).await.unwrap();
    println!("cargo:rerun-if-changed=build.rs");
}
