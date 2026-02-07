use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TaskPriority {
    High,
    Low,
    Lowest,
}

pub type TaskFn = Box<dyn FnOnce() -> std::pin::Pin<Box<dyn Future<Output = ()> + Send>> + Send + 'static>;
pub type TaskFn2<O: Send + Sync> =
    Box<dyn FnOnce() -> std::pin::Pin<Box<dyn Future<Output = O> + Send>> + Send + 'static>;
pub type TaskInnerFn = std::pin::Pin<Box<dyn Future<Output = ()> + Send>>;

pub(crate) struct QueuedTask {
    pub task: TaskFn,
    pub priority: TaskPriority,
    pub cancel: CancellationToken,
}

impl QueuedTask {
    pub fn new(task: TaskFn, priority: TaskPriority, cancel: CancellationToken) -> Self {
        Self {
            task,
            priority,
            cancel,
        }
    }
}

impl PartialEq for QueuedTask {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority
    }
}

impl Eq for QueuedTask {}

impl PartialOrd for QueuedTask {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for QueuedTask {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.priority.cmp(&other.priority)
    }
}
