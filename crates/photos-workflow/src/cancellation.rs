use std::sync::Arc;
use tokio_util::sync::CancellationToken;

type CancellationHandle = Arc<CancellationToken>;
