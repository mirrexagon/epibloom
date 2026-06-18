use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};

/// Calls only the last function set by `call()` after the set duration has
/// passed with no calls to `call()`.
#[derive(Debug)]
pub struct Debounce {
    delay: Duration,
    task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl Debounce {
    /// Create a new debounce with a delay.
    pub fn new(delay: Duration) -> Self {
        Debounce {
            delay,
            task: Arc::new(Mutex::new(None)),
        }
    }

    /// Call the given function after the delay. A subsequent call will override this one.
    pub async fn debounce<F, Fut, Output>(&self, f: F)
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Output> + Send + 'static,
    {
        let mut task = self.task.lock().await;

        // Cancel existing task if it exists.
        if let Some(handle) = task.take() {
            handle.abort();
        }

        // Start a new task with a fresh full delay.
        let delay = self.delay;
        *task = Some(tokio::spawn(async move {
            sleep(delay).await;
            f().await;
        }));
    }
}
