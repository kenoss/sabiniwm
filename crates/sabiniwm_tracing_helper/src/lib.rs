pub mod debug;

/// Filters out span context for event logging
pub struct NoSpanContextFilter;

impl<S> tracing_subscriber::layer::Filter<S> for NoSpanContextFilter
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    fn enabled(
        &self,
        metadata: &tracing_core::Metadata<'_>,
        _cx: &tracing_subscriber::layer::Context<'_, S>,
    ) -> bool {
        !metadata.is_span()
    }
}
