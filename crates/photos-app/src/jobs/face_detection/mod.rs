use crate::jobs::TaskContext;
use crate::jobs::common::ExpandMapReduce;
use crate::jobs::face_detection::detect_faces::DetectFacesTask;
use crate::jobs::face_detection::dispatch_face_detection::DiscoverImagesToDetect;
use photos_domain::{FaceDetection, ImageRecord};
use std::sync::Arc;

mod detect_faces;
pub mod dispatch_face_detection;

pub(crate) fn get_face_detection_job(
    ctx: TaskContext,
) -> ExpandMapReduce<(), ImageRecord, (), ()> {
    ExpandMapReduce {
        expand: Arc::new(DiscoverImagesToDetect { ctx: ctx.clone() }),
        map: Arc::new(DetectFacesTask { ctx: ctx.clone() }),
        reduce: Arc::new(()),
    }
}
