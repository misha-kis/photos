use std::path::{Path, PathBuf};

use cv::{BoundingBox, FaceDetector};
use image::{DynamicImage, Rgba, RgbaImage};
use imageproc::drawing::draw_hollow_rect_mut;
use imageproc::rect::Rect;
use std::fs;

fn save_image_with_boxes(
    img: &DynamicImage,
    boxes: &[BoundingBox],
    output_path: &PathBuf,
) -> image::ImageResult<()> {
    let mut rgba_img: RgbaImage = img.to_rgba8();

    let color = Rgba([255, 0, 0, 255]);

    for &bb in boxes {
        let x1 = bb.x1 as u32;
        let x2 = bb.x2 as u32;
        let y1 = bb.y1 as u32;
        let y2 = bb.y2 as u32;
        let rect =
            Rect::at(x1 as i32, y1 as i32).of_size(x2.saturating_sub(x1), y2.saturating_sub(y1));

        draw_hollow_rect_mut(&mut rgba_img, rect, color);
    }

    rgba_img.save(output_path)
}

fn main() -> anyhow::Result<()> {
    let workspace_path = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let img = image::open(workspace_path.join("test_data").join("example.jpeg"))?;
    let model_path = workspace_path.join("models").join("yolov12n-face.onnx");
    let out_dir = workspace_path.join("runs");
    fs::create_dir_all(&out_dir)?;
    let mut model = FaceDetector::new(model_path, 480)?;
    let result = model.detect(img.clone())?;

    let out_path = out_dir.join("out.png");

    save_image_with_boxes(&img, &result, &out_path)?;

    Ok(())
}
