pub mod queue;
pub mod task;
mod triple_queue;

pub use queue::TaskQueue;
pub use task::{TaskFn, TaskPriority};
