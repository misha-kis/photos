use uuid::Uuid;

#[derive(Copy, Clone, PartialEq)]
pub struct BoundingBox {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl BoundingBox {
    fn x2(&self) -> f32 {
        self.x + self.w
    }

    fn y2(&self) -> f32 {
        self.y + self.h
    }

    pub fn intersection(&self, other: &Self) -> f32 {
        (self.x2().min(other.x2()) - self.x.max(other.x))
            * (self.y2().min(other.y2()) - self.y.max(other.y))
    }

    pub fn union(&self, other: &Self) -> f32 {
        ((self.x2() - self.x) * (self.y2() - self.y))
            + ((other.x2() - other.x) * (other.y2() - other.y))
            - self.intersection(other)
    }
}

#[derive(Clone, Copy)]
pub struct FaceDetection {
    pub uuid: Uuid,
    pub bounding_box: BoundingBox,
    pub confidence: f32,
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
pub struct FaceDetectionWithEmbedding {
    pub detection: FaceDetection,
    pub embedding: [f32; 512], // todo: make generic
}

pub struct ClusteredFaceDetection {
    pub cluster_id: Option<u32>,
    pub detection: FaceDetectionWithEmbedding,
}
