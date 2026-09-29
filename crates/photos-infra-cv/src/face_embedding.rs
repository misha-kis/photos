use crate::{errors::IntoInternal, geometry::transform_image};
use image::DynamicImage;
use ndarray::Array;
use ort::ep::CPU;
use ort::inputs;
use ort::session::{Session, SessionOutputs};
use ort::value::TensorRef;
use photos_domain::{FaceDetection, FaceDetectionWithEmbedding};
use photos_services::ImageAnalysisServiceError;
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
            .with_execution_providers([CPU::default().build()])
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
    ) -> Result<FaceDetectionWithEmbedding, ImageAnalysisServiceError> {
        let aligned = transform_image(
            image,
            &detection.transform,
            (self.image_size, self.image_size),
        )?;

        let mut input = Array::zeros((1, 3, self.image_size as usize, self.image_size as usize));
        for (index, pixel) in aligned.into_raw().as_chunks::<3>().0.iter().enumerate() {
            let y = index / self.image_size as usize;
            let x = index % self.image_size as usize;
            input[[0, 0, y, x]] = (pixel[0] as f32) / 255.;
            input[[0, 1, y, x]] = (pixel[1] as f32) / 255.;
            input[[0, 2, y, x]] = (pixel[2] as f32) / 255.;
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
