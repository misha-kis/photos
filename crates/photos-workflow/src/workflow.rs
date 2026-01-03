use crate::errors::{JobError, StepError};
use crate::progress::WorkflowEvent;
use crate::step::{Step, StepContext};
use futures::StreamExt;
use futures::stream::FuturesUnordered;
use std::sync::Arc;
use tokio::sync::Semaphore;

pub struct Workflow {
    pub steps: Vec<Box<dyn Step>>,
}

pub async fn run_workflow(
    workflow: Workflow,
    ctx: StepContext,
    max_parallel_steps: usize,
) -> Result<(), JobError> {
    tracing::info!("running workflow");
    let _ = ctx
        .progress_reporter
        .send(WorkflowEvent::JobStarted { job_id: ctx.job_id })
        .await;

    let semaphore = Arc::new(Semaphore::new(max_parallel_steps));
    let mut futures = FuturesUnordered::new();

    tracing::debug!("workflow: creating tasks");
    for step in workflow.steps {
        let name = step.name();
        let permit = match semaphore.clone().acquire_owned().await {
            Ok(p) => p,
            Err(_) => {
                return Err(JobError::StepFailed {
                    step: name,
                    error: StepError::Failed("semaphore closed".to_string()),
                });
            }
        };
        let step_ctx = ctx.clone();

        futures.push(tokio::spawn(async move {
            tracing::debug!("workflow: starting step: {name}");
            let _ = step_ctx
                .progress_reporter
                .send(WorkflowEvent::StepStarted {
                    job_id: step_ctx.job_id,
                    step: name,
                })
                .await;

            if step_ctx.cancel.is_cancelled() {
                tracing::debug!("workflow: step cancelled: {name}");
                return Err(StepError::Cancelled);
            }

            let step_ctx = step_ctx.clone();
            let res = step.execute(&step_ctx).await;
            tracing::debug!("workflow: step done: {name}");

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

    tracing::info!("workflow finished");
    let _ = ctx
        .progress_reporter
        .send(WorkflowEvent::JobFinished { job_id: ctx.job_id })
        .await;

    Ok(())
}
