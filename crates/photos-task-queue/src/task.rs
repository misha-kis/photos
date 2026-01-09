#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TaskPriority {
    High,
    Low,
    Lowest,
}

pub type TaskFn =
    Box<dyn FnOnce() -> std::pin::Pin<Box<dyn Future<Output = ()> + Send>> + Send + 'static>;
pub type TaskInnerFn = std::pin::Pin<Box<dyn Future<Output = ()> + Send>>;

pub(crate) struct QueuedTask {
    pub task: TaskFn,
    pub priority: TaskPriority,
}

impl QueuedTask {
    pub fn new(task: TaskFn, priority: TaskPriority) -> Self {
        Self { task, priority }
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
        Some(self.priority.cmp(&other.priority))
    }
}

impl Ord for QueuedTask {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.priority.cmp(&other.priority)
    }
}
