pub struct BoundingBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

pub struct FaceDetection {
    pub bounding_box: BoundingBox,
    pub confidence: f32,
}
pub struct FaceDetectionWithEmbedding {
    pub detection: FaceDetection,
    pub embedding: [f32; 512], // todo: make generic
}
