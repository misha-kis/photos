mod import_item;
use crate::errors::AppError;
use crate::jobs::common::{Expand, ExpandMapReduce, TaskContext};
use crate::jobs::import::import_item::{CopyItemTask, InsertRecordsTask};
use async_trait::async_trait;
use photos_domain::ImageRecord;
use std::path::PathBuf;
use std::sync::Arc;

struct ImportItemsIdentityExpand {}

#[async_trait]
impl Expand<Vec<PathBuf>, PathBuf> for ImportItemsIdentityExpand {
    async fn expand(&self, input: Vec<PathBuf>) -> Result<Vec<PathBuf>, AppError> {
        Ok(input)
    }
}

pub(crate) fn get_import_job(
    ctx: TaskContext,
) -> ExpandMapReduce<Vec<PathBuf>, PathBuf, ImageRecord, ()> {
    ExpandMapReduce {
        expand: Arc::new(ImportItemsIdentityExpand {}),
        map: Arc::new(CopyItemTask { ctx: ctx.clone() }),
        reduce: Arc::new(InsertRecordsTask { ctx }),
    }
}
