// use image::DynamicImage;
//
// use crate::apple_face_detection::Landmarks;
//
// const DST_ALIGNMENT: [[f32; 2]; 5] = [
//     [38.2946, 51.6963],
//     [73.5318, 51.5014],
//     [56.0252, 71.7366],
//     [41.5493, 92.3655],
//     [70.7299, 92.2041],
// ];
//
// pub(crate) fn align_face(
//     image: DynamicImage,
//     landmarks: &Landmarks,
//     out_size: (u32, u32),
// ) -> DynamicImage {
//     let scale_x = out_size.0 as f32 / 112.0;
//     let scale_y = out_size.1 as f32 / 112.0;
//
//     let dst_alignment = DST_ALIGNMENT.map(|[x, y]| [x * scale_x, y * scale_y]);
//     let landmarks_array = [
//         landmarks.left_eye,
//         landmarks.right_eye,
//         landmarks.nose,
//         landmarks.mouth_left,
//         landmarks.mouth_right,
//     ];
// }
