use tokio::task::JoinHandle;

/// A helper function to spawn a blocking thread with tokio and pass in the current span so that the
/// blocking thread has access to the current span as its parent.
pub fn spawn_blocking_with_tracing<F, R>(f: F) -> JoinHandle<R>
where
    // Trait bounds and signature copied from `spawn_blocking`
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    let current_span = tracing::Span::current();
    tokio::task::spawn_blocking(move || current_span.in_scope(f))
}
