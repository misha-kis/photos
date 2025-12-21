use crate::errors::{JobError, StepError};
use crate::progress::WorkflowEvent;
use crate::step::{Step, StepContext};
use futures::StreamExt;
use futures::stream::FuturesUnordered;
use std::sync::Arc;
use tokio::sync::Semaphore;

pub struct Workflow {
    pub steps: Vec<Box<dyn Step>>,
    pub done: u64,
}

pub struct WorkflowRunner {
    max_parallel_steps: usize,
}

impl Default for WorkflowRunner {
    fn default() -> Self {
        Self::new(1)
    }
}

impl WorkflowRunner {
    pub fn new(max_parallel_steps: usize) -> Self {
        Self { max_parallel_steps }
    }

    pub async fn run(&self, workflow: Workflow, ctx: StepContext) -> Result<(), JobError> {
        let _ = ctx
            .progress_reporter
            .send(WorkflowEvent::JobStarted { job_id: ctx.job_id })
            .await;

        let semaphore = Arc::new(Semaphore::new(self.max_parallel_steps));
        let mut futures = FuturesUnordered::new();

        for step in workflow.steps {
            let name = step.name();
            let permit = semaphore.clone().acquire_owned().await.unwrap();
            let step_ctx = ctx.clone();

            futures.push(tokio::spawn(async move {
                let _ = step_ctx
                    .progress_reporter
                    .send(WorkflowEvent::StepStarted {
                        job_id: step_ctx.job_id,
                        step: name,
                    })
                    .await;

                if step_ctx.cancel.is_cancelled() {
                    return Err(StepError::Cancelled);
                }

                let step_ctx = step_ctx.clone();
                let res = step.execute(&step_ctx).await;

                drop(permit);

                res
            }));
        }

        while let Some(result) = futures.next().await {
            match result {
                Ok(Ok(_)) => continue,
                Ok(Err(e)) => {
                    return Err(JobError::StepFailed {
                        step: "unknown",
                        error: e,
                    });
                }
                Err(e) => {
                    return Err(JobError::StepFailed {
                        step: "unknown",
                        error: StepError::Failed(e.to_string()),
                    });
                }
            }
        }

        let _ = ctx
            .progress_reporter
            .send(WorkflowEvent::JobFinished { job_id: ctx.job_id })
            .await;

        Ok(())
    }
}
