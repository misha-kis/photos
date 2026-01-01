use photos_core::JobId;

#[derive(Clone)]
pub struct ProgressReporter {
    pub sender: tokio::sync::mpsc::Sender<WorkflowEvent>,
}

impl ProgressReporter {
    pub async fn send(&self, event: WorkflowEvent) {
        let _ = self.sender.send(event).await;
    }
}

#[derive(Debug)]
pub enum WorkflowEvent {
    JobStarted {
        job_id: JobId,
    },
    StepStarted {
        job_id: JobId,
        step: &'static str,
    },
    StepProgress {
        job_id: JobId,
        step: &'static str,
        current: u64,
        total: u64,
    },
    StepFinished {
        job_id: JobId,
        step: &'static str,
    },
    JobFinished {
        job_id: JobId,
    },
}
