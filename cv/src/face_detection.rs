use std::path::PathBuf;

use anyhow::Result;
use image::{DynamicImage, GenericImageView, imageops::FilterType};
use ndarray::{Array, Axis, s};
use ort::execution_providers::CoreMLExecutionProvider;
use ort::{
    inputs,
    session::{Session, SessionOutputs},
    value::TensorRef,
};

#[derive(Debug, Clone, Copy)]
pub struct BoundingBox {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

impl BoundingBox {
    pub fn new(x1: f32, y1: f32, x2: f32, y2: f32) -> Self {
        Self { x1, y1, x2, y2 }
    }

    pub fn height(&self) -> f32 {
        self.y2 - self.y1
    }

    pub fn width(&self) -> f32 {
        self.x2 - self.x1
    }
}

fn intersection(box1: &BoundingBox, box2: &BoundingBox) -> f32 {
    (box1.x2.min(box2.x2) - box1.x1.max(box2.x1)) * (box1.y2.min(box2.y2) - box1.y1.max(box2.y1))
}

fn union(box1: &BoundingBox, box2: &BoundingBox) -> f32 {
    ((box1.x2 - box1.x1) * (box1.y2 - box1.y1)) + ((box2.x2 - box2.x1) * (box2.y2 - box2.y1))
        - intersection(box1, box2)
}

pub struct FaceDetector {
    session: Session,
    image_size: usize,
}

impl FaceDetector {
    pub fn new(model_path: PathBuf, image_size: usize) -> Result<Self> {
        ort::init()
            .with_execution_providers([CoreMLExecutionProvider::default().build()])
            .commit()?;
        let session = Session::builder()?.commit_from_file(model_path)?;
        Ok(Self {
            session,
            image_size,
        })
    }

    pub fn detect(&mut self, image: DynamicImage) -> Result<Vec<BoundingBox>> {
        let (img_width, img_height) = (image.width(), image.height());
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
            .run(inputs!["images" => TensorRef::from_array_view(&input)?])?;
        let output = outputs["output0"]
            .try_extract_array::<f32>()?
            .t()
            .into_owned();

        let mut boxes = Vec::new();
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
            boxes.push((
                BoundingBox {
                    x1: xc - w / 2.,
                    y1: yc - h / 2.,
                    x2: xc + w / 2.,
                    y2: yc + h / 2.,
                },
                prob,
            ));
        }

        boxes.sort_by(|box1, box2| box2.1.total_cmp(&box1.1));
        let mut result = Vec::new();
        while !boxes.is_empty() {
            result.push(boxes[0].0);
            boxes = boxes
                .iter()
                .filter(|box1| {
                    intersection(&boxes[0].0, &box1.0) / union(&boxes[0].0, &box1.0) < 0.7
                })
                .copied()
                .collect();
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::FaceDetector;
    use std::path::PathBuf;

    fn workspace_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf()
    }

    #[test]
    fn test() {
        let model_path = workspace_path().join("models/yolov12n-face.onnx");
        let mut model = FaceDetector::new(model_path, 480).unwrap();
        let image_path = workspace_path().join("test_data/example.jpeg");
        let image = image::open(image_path).unwrap();
        let out = model.detect(image).unwrap();
        assert_eq!(out.len(), 1);
    }
}
