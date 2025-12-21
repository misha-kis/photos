use std::fmt::Formatter;

#[derive(thiserror::Error, Clone, Debug)]
pub enum WorkflowError {
    Cancelled,
    StepError(StepError),
}

#[derive(thiserror::Error, Clone, Debug)]
pub enum JobError {
    StepFailed {
        step: &'static str,
        error: StepError,
    },
}
#[derive(thiserror::Error, Clone, Debug)]
pub enum StepError {
    Cancelled,
    Failed(String),
}

impl std::fmt::Display for StepError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            StepError::Cancelled => write!(f, "Step Cancelled"),
            StepError::Failed(s) => write!(f, "Step failed: {s}"),
        }
    }
}

impl std::fmt::Display for JobError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let Self::StepFailed { step, error } = self;
        write!(f, "Job failed at step {step} with error {error}")
    }
}

impl std::fmt::Display for WorkflowError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkflowError::Cancelled => write!(f, "Workflow cancelled"),
            WorkflowError::StepError(e) => write!(f, "Workflow failed at step with error {e}"),
        }
    }
}

impl From<StepError> for WorkflowError {
    fn from(value: StepError) -> Self {
        Self::StepError(value)
    }
}
