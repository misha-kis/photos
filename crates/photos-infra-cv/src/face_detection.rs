use std::path::PathBuf;

use crate::errors::IntoInternal;
use image::{DynamicImage, GenericImageView};
use ndarray::{Array, Axis, s};
use ort::ep::CoreMLExecutionProvider;
use ort::{
    inputs,
    session::{Session, SessionOutputs},
    value::TensorRef,
};
use photos_domain::BoundingBox;
use photos_services::{ImageAnalysisServiceError, ResizeService};

#[derive(Copy, Clone)]
pub(crate) struct FaceDetection {
    pub(crate) bounding_box: BoundingBox,
    pub(crate) confidence: f32,
}

impl PartialEq for FaceDetection {
    fn eq(&self, other: &Self) -> bool {
        self.confidence.total_cmp(&other.confidence) == std::cmp::Ordering::Equal
    }
}

impl PartialOrd for FaceDetection {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for FaceDetection {}

impl Ord for FaceDetection {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.confidence.total_cmp(&other.confidence)
    }
}

pub(crate) struct FaceDetector {
    session: Session,
    image_size: u32,
}

impl FaceDetector {
    pub(crate) fn new(
        model_path: PathBuf,
        image_size: u32,
    ) -> Result<Self, ImageAnalysisServiceError> {
        ort::init()
            .with_execution_providers([CoreMLExecutionProvider::default().build()])
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

    pub(crate) fn detect(
        &mut self,
        image: &DynamicImage,
        resize_service: &dyn ResizeService,
    ) -> Result<Vec<FaceDetection>, ImageAnalysisServiceError> {
        let (img_width, img_height) = (image.width(), image.height());
        let img = resize_service
            .resize(image, self.image_size, self.image_size)
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
            .run(inputs!["images" => TensorRef::from_array_view(&input).internal()?])
            .internal()?;
        let output = outputs["output0"]
            .try_extract_array::<f32>()
            .internal()?
            .t()
            .into_owned();

        let mut face_detections = Vec::new();
        let output = output.slice(s![.., .., 0]);
        for row in output.axis_iter(Axis(0)) {
            let row: Vec<_> = row.iter().copied().collect();
            let (_, prob) = row
                .iter()
                .skip(4)
                .enumerate()
                .map(|(index, value)| (index, *value))
                .reduce(|accum, row| if row.1 > accum.1 { row } else { accum })
                .unwrap();
            if prob < 0.5 {
                continue;
            }
            let xc = row[0] / self.image_size as f32 * (img_width as f32);
            let yc = row[1] / self.image_size as f32 * (img_height as f32);
            let w = row[2] / self.image_size as f32 * (img_width as f32);
            let h = row[3] / self.image_size as f32 * (img_height as f32);
            face_detections.push(FaceDetection {
                bounding_box: BoundingBox {
                    x: xc - w / 2.,
                    y: yc - h / 2.,
                    w,
                    h,
                },
                confidence: prob,
            });
        }
        face_detections.sort();
        let mut result = Vec::new();
        while !face_detections.is_empty() {
            result.push(face_detections[0]);
            face_detections = face_detections
                .iter()
                .filter(|box1| {
                    face_detections[0]
                        .bounding_box
                        .intersection(&box1.bounding_box)
                        / face_detections[0].bounding_box.union(&box1.bounding_box)
                        < 0.7
                })
                .copied()
                .collect();
        }
        Ok(result)
    }
}
