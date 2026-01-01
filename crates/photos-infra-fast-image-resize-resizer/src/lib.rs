use fast_image_resize::PixelType::{U8, U8x2, U8x3, U8x4};
use fast_image_resize::{IntoImageView, Resizer};
use image::{DynamicImage, GrayAlphaImage, GrayImage, RgbImage, RgbaImage};
use photos_services::{ResizeService, ResizeServiceError};
use std::sync::Mutex;

pub struct FastImageResizeResizer {
    resizer: Mutex<Resizer>,
}

impl Default for FastImageResizeResizer {
    fn default() -> Self {
        Self {
            resizer: Mutex::new(Resizer::default()),
        }
    }
}

impl ResizeService for FastImageResizeResizer {
    fn resize(
        &self,
        image: &DynamicImage,
        width: u32,
        height: u32,
    ) -> Result<DynamicImage, ResizeServiceError> {
        if width == 0 || height == 0 {
            return Err(ResizeServiceError::ResizeServiceError);
        }
        let mut dst_image = fast_image_resize::images::Image::new(
            width,
            height,
            image.pixel_type().expect("has pixel type"),
        );

        self.resizer
            .lock()
            .expect("can acquire lock")
            .resize(image, &mut dst_image, None)
            .map_err(|_| ResizeServiceError::ResizeServiceError)?;

        let buffer = dst_image.buffer().to_vec();
        Ok(match dst_image.pixel_type() {
            U8 => DynamicImage::ImageLuma8(
                GrayImage::from_raw(width, height, buffer)
                    .ok_or(ResizeServiceError::ResizeServiceError)?,
            ),
            U8x2 => DynamicImage::ImageLumaA8(
                GrayAlphaImage::from_raw(width, height, buffer)
                    .ok_or(ResizeServiceError::ResizeServiceError)?,
            ),
            U8x3 => DynamicImage::ImageRgb8(
                RgbImage::from_raw(width, height, buffer)
                    .ok_or(ResizeServiceError::ResizeServiceError)?,
            ),
            U8x4 => DynamicImage::ImageRgba8(
                RgbaImage::from_raw(width, height, buffer)
                    .ok_or(ResizeServiceError::ResizeServiceError)?,
            ),
            _ => Err(ResizeServiceError::ResizeServiceError)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, RgbImage};

    fn create_test_image(width: u32, height: u32) -> DynamicImage {
        let mut img = RgbImage::new(width, height);

        for x in 0..width {
            for y in 0..height {
                let pixel = image::Rgb([(x % 255) as u8, (y % 255) as u8, ((x + y) % 255) as u8]);
                img.put_pixel(x, y, pixel);
            }
        }

        DynamicImage::ImageRgb8(img)
    }

    #[test]
    fn test_new_creates_resizer() {
        let resizer = FastImageResizeResizer::default();
        assert_eq!(std::mem::size_of_val(&resizer.resizer) > 0, true);
    }

    #[test]
    fn test_resize_to_smaller_dimensions() {
        let mut resizer = FastImageResizeResizer::default();
        let original_image = create_test_image(100, 100);

        let result = resizer.resize(&original_image, 50, 50);

        assert!(result.is_ok());
        let resized = result.unwrap();
        assert_eq!(resized.width(), 50);
        assert_eq!(resized.height(), 50);
        assert!(resized.as_bytes().len() > 0);
    }

    #[test]
    fn test_resize_to_larger_dimensions() {
        let mut resizer = FastImageResizeResizer::default();
        let original_image = create_test_image(50, 50);

        let result = resizer.resize(&original_image, 150, 150);

        assert!(result.is_ok());
        let resized = result.unwrap();
        assert_eq!(resized.width(), 150);
        assert_eq!(resized.height(), 150);
    }

    #[test]
    fn test_resize_to_same_dimensions() {
        let mut resizer = FastImageResizeResizer::default();
        let original_image = create_test_image(80, 80);

        let result = resizer.resize(&original_image, 80, 80);

        assert!(result.is_ok());
        let resized = result.unwrap();
        assert_eq!(resized.width(), 80);
        assert_eq!(resized.height(), 80);
    }

    #[test]
    fn test_resize_preserves_aspect_ratio_not_required() {
        let mut resizer = FastImageResizeResizer::default();
        let original_image = create_test_image(100, 50);

        let result = resizer.resize(&original_image, 50, 50);

        assert!(result.is_ok());
        let resized = result.unwrap();
        assert_eq!(resized.width(), 50);
        assert_eq!(resized.height(), 50);
    }

    #[test]
    fn test_resize_with_zero_width_or_height() {
        let mut resizer = FastImageResizeResizer::default();
        let original_image = create_test_image(100, 100);

        let result = resizer.resize(&original_image, 0, 100);
        assert!(result.is_err());

        let result = resizer.resize(&original_image, 100, 0);
        assert!(result.is_err());

        let result = resizer.resize(&original_image, 0, 0);
        assert!(result.is_err());
    }

    #[test]
    fn test_resize_different_image_formats() {
        let mut resizer = FastImageResizeResizer::default();

        let gray_img = image::GrayImage::new(100, 100);
        let dynamic_gray = DynamicImage::ImageLuma8(gray_img);

        let result = resizer.resize(&dynamic_gray, 50, 50);
        assert!(result.is_ok());

        let rgba_img = image::RgbaImage::new(100, 100);
        let dynamic_rgba = DynamicImage::ImageRgba8(rgba_img);

        let result = resizer.resize(&dynamic_rgba, 50, 50);
        assert!(result.is_ok());
    }

    #[test]
    fn test_resizer_reuse() {
        let mut resizer = FastImageResizeResizer::default();

        let img1 = create_test_image(100, 100);
        let result1 = resizer.resize(&img1, 50, 50);
        assert!(result1.is_ok());

        let img2 = create_test_image(200, 200);
        let result2 = resizer.resize(&img2, 75, 75);
        assert!(result2.is_ok());

        assert_eq!(result1.unwrap().width(), 50);
        assert_eq!(result2.unwrap().width(), 75);
    }

    #[test]
    fn test_resize_pixel_data_integrity() {
        let mut resizer = FastImageResizeResizer::default();

        let mut img = RgbImage::new(2, 2);
        img.put_pixel(0, 0, image::Rgb([255, 0, 0]));
        img.put_pixel(1, 0, image::Rgb([0, 255, 0]));
        img.put_pixel(0, 1, image::Rgb([0, 0, 255]));
        img.put_pixel(1, 1, image::Rgb([255, 255, 255]));

        let original = DynamicImage::ImageRgb8(img);

        let result = resizer.resize(&original, 2, 2);
        assert!(result.is_ok());

        let resized = result.unwrap();
        assert_eq!(resized.width(), 2);
        assert_eq!(resized.height(), 2);

        let pixel_count = resized.width() * resized.height();
        assert_eq!(resized.as_bytes().len() as u32, pixel_count * 3);
    }
}
