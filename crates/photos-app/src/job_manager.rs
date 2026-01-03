use dashmap::DashMap;
use photos_core::JobId;
use photos_workflow::{Job, JobState, StepContext, Workflow, run_workflow};
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct JobManager {
    jobs: DashMap<JobId, Job>,
}

impl JobManager {
    pub fn new() -> Self {
        Self {
            jobs: DashMap::new(),
        }
    }

    pub fn cancel(&self, id: JobId) {
        if let Some(job) = self.jobs.get(&id) {
            job.cancel.cancel();
        }
    }

    pub fn spawn_workflow(&self, workflow: Workflow, ctx: StepContext) -> JobId {
        let job_id = JobId::new_v4();
        let cancel = ctx.cancel.clone();
        let job_state = Arc::new(Mutex::new(JobState::Pending));

        self.jobs.insert(
            job_id,
            Job {
                id: job_id,
                state: job_state.clone(),
                cancel,
            },
        );

        tokio::spawn(async move {
            *job_state.lock().await = JobState::Running;
            let result = run_workflow(workflow, ctx, 1).await;
            let final_state = match result {
                Ok(()) => JobState::Completed,
                Err(e) => JobState::Failed(e),
            };
            *job_state.lock().await = final_state;
        });

        job_id
    }
}
