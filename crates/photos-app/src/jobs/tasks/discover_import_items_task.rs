use async_trait::async_trait;
use photos_infra_import_item_discovery::discover_import_items;
use std::path::PathBuf;

use crate::{AppError, jobs::common::Map};

pub(crate) struct DiscoverImportItemsTask {}

#[async_trait]
impl Map<PathBuf, Vec<PathBuf>> for DiscoverImportItemsTask {
    async fn map(&self, path: PathBuf) -> Result<Vec<PathBuf>, AppError> {
        Ok(discover_import_items(path))
    }
}
