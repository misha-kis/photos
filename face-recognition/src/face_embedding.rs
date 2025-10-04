use std::path::Path;

use anyhow::Result;
use candle_core::Tensor;
use image::DynamicImage;
use objc2::{
    rc::{Retained, autoreleasepool},
    runtime::ProtocolObject,
};
use objc2_core_ml::{MLDictionaryFeatureProvider, MLFeatureProvider, MLModel};
use objc2_foundation::{NSString, NSURL};

use crate::utils::{create_feature_provider, extract_vector, tensor_to_mlmultiarray};

const INPUT_NAME: &str = "x_1";
const OUTPUT_NAME: &str = "var_2167";

pub struct FaceEmbeddingModel {
    model: Retained<MLModel>,
}

impl FaceEmbeddingModel {
    pub fn new(path: &Path) -> Result<Self> {
        if !path.exists() {
            anyhow::bail!("Model file not found: {}", path.display());
        }

        autoreleasepool(|_| {
            let url =
                unsafe { NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy())) };
            match unsafe { MLModel::modelWithContentsOfURL_error(&url) } {
                Ok(model) => Ok(model),
                Err(err) => Err(err.into()),
            }
        })
        .map(|model| Self { model })
    }

    pub fn generate_embedding(&self, image: DynamicImage) -> Result<Vec<f32>> {
        let tensor = tensor_from_image(image)?;
        let ml_array = tensor_to_mlmultiarray(&tensor)?;
        let provider = create_feature_provider(INPUT_NAME, &ml_array)?;
        let prediction = run_model_prediction(&self.model, &provider)?;
        let embedding = extract_vector(&prediction, OUTPUT_NAME)?;
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

fn run_model_prediction(
    model: &MLModel,
    provider: &MLDictionaryFeatureProvider,
) -> Result<Retained<ProtocolObject<dyn MLFeatureProvider>>> {
    objc2::rc::autoreleasepool(|_| unsafe {
        // Convert MLDictionaryFeatureProvider to ProtocolObject
        let protocol_provider = ProtocolObject::from_ref(provider);

        model
            .predictionFromFeatures_error(protocol_provider)
            .map_err(|e| e.into())
    })
}
