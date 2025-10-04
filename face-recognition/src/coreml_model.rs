use std::path::Path;

use anyhow::Result;
use candle_core::Tensor;
use objc2::{
    rc::{Retained, autoreleasepool},
    runtime::ProtocolObject,
};
use objc2_core_ml::{MLDictionaryFeatureProvider, MLFeatureProvider, MLModel};
use objc2_foundation::{NSString, NSURL};

use crate::utils::{create_feature_provider, extract_vector, tensor_to_mlmultiarray};

pub struct CoreMLModel {
    model: Retained<MLModel>,
    input_name: String,
    output_name: String,
}
impl CoreMLModel {
    pub fn new(path: &Path, input_name: String, output_name: String) -> Result<Self> {
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
        .map(|model| Self {
            model,
            input_name,
            output_name,
        })
    }

    pub fn predict(&self, tensor: Tensor) -> Result<Vec<f32>> {
        let ml_array = tensor_to_mlmultiarray(&tensor)?;
        let provider = create_feature_provider(&self.input_name, &ml_array)?;
        let prediction = run_model_prediction(&self.model, &provider)?;
        let embedding = extract_vector(&prediction, &self.output_name)?;
        Ok(embedding)
    }
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
