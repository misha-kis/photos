use std::sync::Arc;

use crate::errors::StepError;
use crate::progress::ProgressReporter;
use photos_core::{JobId, Uuid};
use photos_services::ServiceRegistry;
use tokio_util::sync::CancellationToken;

pub type StepId = Uuid;

#[derive(Clone)]
pub struct StepContext {
    pub job_id: JobId,
    pub cancel: CancellationToken,
    pub progress_reporter: ProgressReporter,
    pub services: Arc<dyn ServiceRegistry>,
}

#[async_trait::async_trait]
pub trait Step: Send + Sync {
    async fn execute(&self, ctx: &StepContext) -> Result<(), StepError>;

    fn name(&self) -> &'static str;
}
