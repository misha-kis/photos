use uuid::Uuid;

#[derive(Copy, Clone, PartialEq)]
pub struct BoundingBox {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl BoundingBox {
    fn x2(&self) -> f64 {
        self.x + self.w
    }

    fn y2(&self) -> f64 {
        self.y + self.h
    }

    pub fn intersection(&self, other: &Self) -> f64 {
        (self.x2().min(other.x2()) - self.x.max(other.x))
            * (self.y2().min(other.y2()) - self.y.max(other.y))
    }

    pub fn union(&self, other: &Self) -> f64 {
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
    pub transform: Affine2D,
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

#[derive(Debug, Clone, Copy)]
pub struct Affine2D {
    // [ a b tx ]
    // [ c d ty ]
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub tx: f64,
    pub ty: f64,
}
impl Affine2D {
    pub fn as_kornia(&self) -> [f64; 6] {
        [self.a, self.b, self.tx, self.c, self.d, self.ty]
    }

    pub fn transform(&self, x: f64, y: f64) -> (f64, f64) {
        (
            self.a * x + self.b * y + self.tx,
            self.c * x + self.d * y + self.ty,
        )
    }

    pub fn inverse(&self) -> Option<Self> {
        let det = self.a * self.d - self.b * self.c;

        if det.abs() < f64::EPSILON {
            return None;
        }

        let inv_a = self.d / det;
        let inv_b = -self.b / det;
        let inv_c = -self.c / det;
        let inv_d = self.a / det;

        Some(Self {
            a: inv_a,
            b: inv_b,
            c: inv_c,
            d: inv_d,
            tx: -(inv_a * self.tx + inv_b * self.ty),
            ty: -(inv_c * self.tx + inv_d * self.ty),
        })
    }

    pub fn adjust_for_padding(&mut self, x: f64, y: f64) {
        self.tx -= self.a * x + self.b * y;
        self.ty -= self.c * x + self.d * y;
    }
}
