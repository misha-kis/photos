use crate::jobs::TaskContext;
use crate::jobs::common::ExpandMapReduce;
use crate::jobs::embedding_generation::cluster_embeddings::ClusterEmbeddings;
use crate::jobs::embedding_generation::dispatch_embedding_generation::DiscoverImagesWithoutEmbeddings;
use crate::jobs::embedding_generation::generate_embeddings::GenerateEmbeddings;
use photos_domain::{FaceDetection, ImageRecord};
use std::sync::Arc;

mod cluster_embeddings;
pub mod dispatch_embedding_generation;
mod generate_embeddings;

pub(crate) fn get_embeddings_detection_job(
    ctx: TaskContext,
) -> ExpandMapReduce<(), (ImageRecord, FaceDetection), (), ()> {
    ExpandMapReduce {
        expand: Arc::new(DiscoverImagesWithoutEmbeddings { ctx: ctx.clone() }),
        map: Arc::new(GenerateEmbeddings { ctx: ctx.clone() }),
        reduce: Arc::new(ClusterEmbeddings { ctx }),
    }
}
