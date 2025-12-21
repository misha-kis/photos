pub enum ProgressEvent {
    Started {
        step: &'static str,
    },
    Progress {
        step: &'static str,
        current: u64,
        total: u64,
    },
    Finished {
        step: &'static str,
    },
    Error {
        step: &'static str,
        error: String,
    },
    Cancelled {
        step: &'static str,
    },
}

pub type Uuid = uuid::Uuid;

pub type JobId = Uuid;

pub enum JobStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}
