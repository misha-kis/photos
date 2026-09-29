use photos_domain::Affine2D;
use photos_domain::BoundingBox;
use photos_services::ImageAnalysisServiceError;

#[derive(Debug, Clone, Copy)]
pub(crate) struct Point {
    pub(crate) x: f64,
    pub(crate) y: f64,
}

impl Point {
    pub(crate) fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

impl From<(f64, f64)> for Point {
    fn from((x, y): (f64, f64)) -> Self {
        Self { x, y }
    }
}
//
// pub struct BoundingBox {
//     pub x: f64,
//     pub y: f64,
//     pub w: f64,
//     pub h: f64,
// }

pub(crate) struct Landmarks {
    pub(crate) left_eye: Point,
    pub(crate) right_eye: Point,
    pub(crate) nose: Point,
    pub(crate) mouth_left: Point,
    pub(crate) mouth_right: Point,
}

fn estimate_similarity_transform(src: &[Point], dst: &[Point]) -> Option<Affine2D> {
    if src.len() != dst.len() || src.len() < 2 {
        return None;
    }

    let n = src.len() as f64;

    let src_cx = src.iter().map(|p| p.x).sum::<f64>() / n;
    let src_cy = src.iter().map(|p| p.y).sum::<f64>() / n;

    let dst_cx = dst.iter().map(|p| p.x).sum::<f64>() / n;
    let dst_cy = dst.iter().map(|p| p.y).sum::<f64>() / n;

    let mut denom = 0.0;
    let mut a = 0.0;
    let mut b = 0.0;

    for (&Point { x: sx, y: sy }, &Point { x: dx, y: dy }) in src.iter().zip(dst.iter()) {
        let sx = sx - src_cx;
        let sy = sy - src_cy;

        let dx = dx - dst_cx;
        let dy = dy - dst_cy;

        denom += sx * sx + sy * sy;

        a += sx * dx + sy * dy;
        b += sx * dy - sy * dx;
    }

    if denom.abs() < f64::EPSILON {
        return None;
    }

    a /= denom;
    b /= denom;

    Some(Affine2D {
        a,
        b: -b,
        c: b,
        d: a,

        tx: dst_cx - a * src_cx + b * src_cy,
        ty: dst_cy - b * src_cx - a * src_cy,
    })
}

pub(crate) fn calculate_alignment_matrix(
    landmarks: &Landmarks,
    image_width: u32,
    image_height: u32,
    output_size: (u32, u32),
) -> Result<Affine2D, ImageAnalysisServiceError> {
    let (output_width, output_height) = output_size;

    let sx = output_width as f64 / 112.0;
    let sy = output_height as f64 / 112.0;

    let src = [
        landmarks.left_eye,
        landmarks.right_eye,
        landmarks.nose,
        landmarks.mouth_left,
        landmarks.mouth_right,
    ];

    let dst = [
        Point::new(38.2946 * sx, 51.6963 * sy),
        Point::new(73.5318 * sx, 51.5014 * sy),
        Point::new(56.0252 * sx, 71.7366 * sy),
        Point::new(41.5493 * sx, 92.3655 * sy),
        Point::new(70.7299 * sx, 92.2041 * sy),
    ];

    let mut m = estimate_similarity_transform(&src, &dst)
        .ok_or(ImageAnalysisServiceError::AffineMatrixCalculationFailed)?;

    // Destination corners.
    let corners = [
        (0.0, 0.0),
        ((output_width - 1) as f64, 0.0),
        ((output_width - 1) as f64, (output_height - 1) as f64),
        (0.0, (output_height - 1) as f64),
    ];

    // Destination -> source.
    let inv = m
        .inverse()
        .ok_or(ImageAnalysisServiceError::AffineMatrixCalculationFailed)?;

    let source_corners: Vec<_> = corners.iter().map(|&(x, y)| inv.transform(x, y)).collect();

    let min_x = source_corners
        .iter()
        .map(|p| p.0)
        .fold(f64::INFINITY, f64::min)
        .floor() as i64;

    let min_y = source_corners
        .iter()
        .map(|p| p.1)
        .fold(f64::INFINITY, f64::min)
        .floor() as i64;

    let max_x = source_corners
        .iter()
        .map(|p| p.0)
        .fold(f64::NEG_INFINITY, f64::max)
        .ceil() as i64;

    let max_y = source_corners
        .iter()
        .map(|p| p.1)
        .fold(f64::NEG_INFINITY, f64::max)
        .ceil() as i64;

    let width = image_width as i64;
    let height = image_height as i64;

    let pad_left = (-min_x).max(0) as f64;
    let pad_top = (-min_y).max(0) as f64;

    let _pad_right = (max_x - width + 1).max(0);

    let _pad_bottom = (max_y - height + 1).max(0);

    // Same coordinate-system correction as your Python code.
    m.adjust_for_padding(pad_left, pad_top);

    Ok(m)
}
