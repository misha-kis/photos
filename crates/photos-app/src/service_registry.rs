use photos_infra_cv::ImageAnalysis;
use photos_infra_fast_image_resize_resizer::FastImageResizeResizer;
use photos_infra_fs_repository::FSImageRepository;
use photos_infra_sqlite_image_metadata_repository::SqliteImageMetadataRepository;
use std::sync::Arc;

pub struct AppServiceRegistry {
    pub image_repository: Arc<FSImageRepository<FastImageResizeResizer>>,
    pub image_metadata_repository: Arc<SqliteImageMetadataRepository>,
    pub resize_service: Arc<FastImageResizeResizer>,
    pub analysis_service: Arc<ImageAnalysis>,
}
