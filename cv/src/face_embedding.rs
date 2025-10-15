use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView};
use ndarray::Array;
use ort::execution_providers::CoreMLExecutionProvider;
use ort::inputs;
use ort::session::{Session, SessionOutputs};
use ort::value::TensorRef;
use std::path::PathBuf;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

pub struct FaceEmbedder {
    session: Session,
    image_size: usize,
}

impl FaceEmbedder {
    pub fn new(model_path: PathBuf, image_size: usize) -> anyhow::Result<Self> {
        tracing_subscriber::registry()
            .with(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "info,ort=debug".into()),
            )
            .with(tracing_subscriber::fmt::layer())
            .init();

        ort::init()
            .with_execution_providers([CoreMLExecutionProvider::default().build()])
            .commit()?;
        let session = Session::builder()?.commit_from_file(model_path)?;
        Ok(Self {
            session,
            image_size,
        })
    }

    pub fn generate_embedding(&mut self, image: DynamicImage) -> anyhow::Result<[f32; 512]> {
        let img = image.resize_exact(
            self.image_size as u32,
            self.image_size as u32,
            FilterType::CatmullRom,
        );
        let mut input = Array::zeros((1, 3, self.image_size, self.image_size));
        for pixel in img.pixels() {
            let x = pixel.0 as _;
            let y = pixel.1 as _;
            let [r, g, b, _] = pixel.2.0;
            input[[0, 0, y, x]] = (r as f32) / 255.;
            input[[0, 1, y, x]] = (g as f32) / 255.;
            input[[0, 2, y, x]] = (b as f32) / 255.;
        }
        let outputs: SessionOutputs = self
            .session
            .run(inputs!["input" => TensorRef::from_array_view(&input)?])?;
        let array = outputs["output"].try_extract_array::<f32>()?;
        assert_eq!(array.len(), 512);
        let mut result = [0f32; 512];
        result.copy_from_slice(
            array
                .as_slice()
                .ok_or(anyhow::anyhow!("array not contiguous"))?,
        );

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn workspace_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf()
    }

    #[test]
    fn test_generate_embedding() {
        let model_path = workspace_path().join("models/facenet.onnx");
        let mut model = FaceEmbedder::new(model_path, 160).unwrap();
        let image_path = workspace_path().join("test_data/example.jpeg");
        let image = image::open(image_path).unwrap();
        let embedding = model.generate_embedding(image).unwrap();
        assert_eq!(embedding.len(), 512);
    }
}
