use std::sync::Arc;
use rayon::ThreadPool;

pub struct RuntimePools {
    pub cpu_pool: Arc<ThreadPool>,
    pub ml_pool: Arc<ThreadPool>,
}

impl RuntimePools {
    pub fn new() -> Self {
        let cpu_pool = Arc::new(
            rayon::ThreadPoolBuilder::new()
                .thread_name(|i| format!("cpu-{}", i))
                .num_threads(num_cpus::get().saturating_sub(1))
                .build()
                .unwrap(),
        );

        let ml_pool = Arc::new(
            rayon::ThreadPoolBuilder::new()
                .thread_name(|i| format!("ml-{}", i))
                .num_threads(2)
                .build()
                .unwrap(),
        );

        Self { cpu_pool, ml_pool }
    }
}
