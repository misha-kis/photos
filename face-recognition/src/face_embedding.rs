use std::path::Path;

use anyhow::Result;
use candle_core::Tensor;
use image::DynamicImage;

use crate::coreml_model::CoreMLModel;

const INPUT_NAME: &str = "x_1";
const OUTPUT_NAME: &str = "var_2167";

pub struct FaceEmbeddingModel {
    model: CoreMLModel,
}

impl FaceEmbeddingModel {
    pub fn new(path: &Path) -> Result<Self> {
        Ok(Self {
            model: CoreMLModel::new(path, INPUT_NAME.into(), OUTPUT_NAME.into())?,
        })
    }

    pub fn generate_embedding(&self, image: DynamicImage) -> Result<Vec<f32>> {
        let tensor = tensor_from_image(image)?;
        let embedding = self.model.predict(tensor)?;
        Ok(embedding)
    }
}

fn tensor_from_image(image: DynamicImage) -> Result<Tensor> {
    let image = image.to_rgb8();
    let data: Vec<f32> = image
        .pixels()
        .flat_map(|p| p.0.iter().map(|&v| v as f32 / 255.0))
        .collect();
    let f32_tensor = Tensor::from_vec(data, (1, 3, 160, 160), &candle_core::Device::Cpu)?;

    let ones = Tensor::ones_like(&f32_tensor)?;

    let normalized_tensor = (f32_tensor / 127.5)? - ones;

    normalized_tensor.map_err(|e| e.into())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn workspace_path() -> std::path::PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf()
    }

    #[test]
    fn test_tensor_from_image() {
        let image_path = workspace_path().join("test_data/example.png");
        let image = image::open(image_path).unwrap();
        let tensor = tensor_from_image(image).unwrap();
        assert_eq!(tensor.dims(), &[1, 3, 160, 160]);
    }

    #[test]
    fn test_generate_embedding() {
        let model_path = workspace_path().join("models/facenet-1.mlmodelc");
        let model = FaceEmbeddingModel::new(model_path.as_path()).unwrap();
        let image_path = workspace_path().join("test_data/example.png");
        let image = image::open(image_path).unwrap();
        let embedding = model.generate_embedding(image).unwrap();
        assert_eq!(embedding.len(), 512);
    }
}
