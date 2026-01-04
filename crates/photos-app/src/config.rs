pub struct Config {
    pub thumbnail_sizes: Vec<u32>,
    pub max_blocking_tasks: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            thumbnail_sizes: vec![128],
            max_blocking_tasks: std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4),
        }
    }
}