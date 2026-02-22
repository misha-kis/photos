use crate::errors::IntoInternal;
use image::{DynamicImage, GenericImageView};
use ndarray::Array;
use ort::ep::{CPUExecutionProvider, CoreMLExecutionProvider};
use ort::inputs;
use ort::session::{Session, SessionOutputs};
use ort::value::TensorRef;
use photos_domain::{FaceDetection, FaceDetectionWithEmbedding};
use photos_services::{ImageAnalysisServiceError, ResizeService};
use std::path::PathBuf;

pub(crate) struct FaceEmbedder {
    session: Session,
    image_size: u32,
}

impl FaceEmbedder {
    pub(crate) fn new(
        model_path: PathBuf,
        image_size: u32,
    ) -> Result<Self, ImageAnalysisServiceError> {
        ort::init()
            .with_execution_providers([
                CoreMLExecutionProvider::default().build(),
                CPUExecutionProvider::default().build(),
            ])
            .commit();
        let session = Session::builder()
            .internal()?
            .commit_from_file(model_path)
            .internal()?;
        Ok(Self {
            session,
            image_size,
        })
    }

    pub(crate) fn generate_embedding(
        &mut self,
        image: &DynamicImage,
        detection: FaceDetection,
        resize_service: &dyn ResizeService,
    ) -> Result<FaceDetectionWithEmbedding, ImageAnalysisServiceError> {
        let image = image.crop_imm(
            detection.bounding_box.x as u32,
            detection.bounding_box.y as u32,
            detection.bounding_box.w as u32,
            detection.bounding_box.h as u32,
        );
        let img = resize_service
            .resize(&image, self.image_size, self.image_size)
            .internal()?;
        let mut input = Array::zeros((1, 3, self.image_size as usize, self.image_size as usize));
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
            .run(inputs!["input" => TensorRef::from_array_view(&input).internal()?])
            .internal()?;
        let array = outputs["output"].try_extract_array::<f32>().internal()?;
        assert_eq!(array.len(), 512);
        let mut embedding = [0f32; 512];
        embedding.copy_from_slice(
            array
                .as_slice()
                .ok_or(ImageAnalysisServiceError::CouldNotInfer)?,
        );

        Ok(FaceDetectionWithEmbedding {
            detection,
            embedding,
        })
    }
}
