use crate::{
    errors::IntoInternal,
    geometry::{Landmarks, Point, calculate_alignment_matrix},
};

use photos_domain::{BoundingBox, FaceDetection};

use apple_vision::{
    BoundingBox as AppleBoundingBox, face_landmarks::detect_face_landmarks_in_path,
};
use photos_services::ImageAnalysisServiceError;

fn convert_bounding_box(bounding_box: AppleBoundingBox, width: f64, height: f64) -> BoundingBox {
    let x = bounding_box.x * width;
    let y = bounding_box.y * height;
    let w = bounding_box.width * width;
    let h = bounding_box.height * height;
    let y = height - y - h;
    BoundingBox { x, y, w, h }
}

fn centroid(landmarks: &[apple_vision::LandmarkPoint]) -> (f64, f64) {
    let x = landmarks.iter().map(|l| l.x).sum::<f64>() / landmarks.len() as f64;
    let y = landmarks.iter().map(|l| l.y).sum::<f64>() / landmarks.len() as f64;
    (x, y)
}

fn convert_landmarks(detection: &apple_vision::FaceWithLandmarks, bbox: &BoundingBox) -> Landmarks {
    let left_eye = Point::from(centroid(&detection.left_eye));
    let right_eye = Point::from(centroid(&detection.right_eye));
    let nose = Point::from(centroid(&detection.nose));
    let mouth_left = detection
        .outer_lips
        .iter()
        .map(|l| Point::new(l.x, l.y))
        .fold(Point::new(f64::MAX, 0f64), |prev, cur| {
            if cur.x < prev.x { cur } else { prev }
        });
    let mouth_right = detection
        .outer_lips
        .iter()
        .map(|l| Point::new(l.x, l.y))
        .fold(Point::new(f64::MIN, 0f64), |prev, cur| {
            if cur.x > prev.x { cur } else { prev }
        });
    let mut landmarks = Landmarks {
        left_eye,
        right_eye,
        nose,
        mouth_left,
        mouth_right,
    };

    landmarks.left_eye.x = landmarks.left_eye.x * bbox.w + bbox.x;
    landmarks.right_eye.x = landmarks.right_eye.x * bbox.w + bbox.x;
    landmarks.nose.x = landmarks.nose.x * bbox.w + bbox.x;
    landmarks.mouth_left.x = landmarks.mouth_left.x * bbox.w + bbox.x;
    landmarks.mouth_right.x = landmarks.mouth_right.x * bbox.w + bbox.x;
    landmarks.left_eye.y = (1. - landmarks.left_eye.y) * bbox.h + bbox.y;
    landmarks.right_eye.y = (1. - landmarks.right_eye.y) * bbox.h + bbox.y;
    landmarks.nose.y = (1. - landmarks.nose.y) * bbox.h + bbox.y;
    landmarks.mouth_left.y = (1. - landmarks.mouth_left.y) * bbox.h + bbox.y;
    landmarks.mouth_right.y = (1. - landmarks.mouth_right.y) * bbox.h + bbox.y;
    landmarks
}

pub(crate) fn detect(image_path: &str) -> Result<Vec<FaceDetection>, ImageAnalysisServiceError> {
    let detections = detect_face_landmarks_in_path(image_path).internal()?;
    let image = image::open(image_path).internal()?;
    let width = image.width();
    let height = image.height();
    let mut results = Vec::new();
    for detection in detections {
        let uuid = photos_domain::Uuid::now_v7();
        let bounding_box =
            convert_bounding_box(detection.bounding_box, width as f64, height as f64);

        let landmarks = convert_landmarks(&detection, &bounding_box);
        let transform = calculate_alignment_matrix(&landmarks, width, height, (112, 112))?;
        results.push(FaceDetection {
            uuid,
            bounding_box,
            confidence: detection.confidence,
            transform,
        });
    }

    Ok(results)
}
