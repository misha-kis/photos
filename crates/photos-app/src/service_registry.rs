use std::sync::Arc;
use photos_infra_fast_image_resize_resizer::FastImageResizeResizer;
use photos_infra_fs_repository::FSImageRepository;
use photos_infra_sqlite_image_metadata_repository::SqliteImageMetadataRepository;

pub struct AppServiceRegistry
{
    pub image_repository: Arc<FSImageRepository<FastImageResizeResizer>>,
    pub image_metadata_repository: Arc<SqliteImageMetadataRepository>,
    pub resize_service: Arc<FastImageResizeResizer>,
}

impl photos_services::ServiceRegistry
    for AppServiceRegistry
{
    fn image_repo(&self) -> &dyn photos_services::ImageRepository {
        self.image_repository.as_ref()
    }
    
    fn image_meta_repo(&self) -> &dyn photos_services::ImageMetadataRepository {
        self.image_metadata_repository.as_ref()
    }

    fn resize_service(&self) -> &dyn photos_services::ResizeService {
        self.resize_service.as_ref()
    }
}

