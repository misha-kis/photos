use crate::errors::JobError;
use photos_core::JobId;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Debug)]
pub enum JobState {
    Pending,
    Running,
    Completed,
    Failed(JobError),
    Cancelled,
}

#[derive(Clone, Debug)]
pub enum JobResult {
    Success,
    Cancelled,
    Fail,
}

pub struct Job {
    pub id: JobId,
    pub state: Arc<Mutex<JobState>>,
    pub cancel: CancellationToken,
}
