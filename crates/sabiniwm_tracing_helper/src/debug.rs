use sabiniwm::SabiniwmState;
use sabiniwm::action::ActionFnI;
use std::sync::{Arc, Mutex};

/// A filter that is able to enable/disable/toggle
pub struct ToggleFilter<S> {
    inner: tracing_subscriber::reload::Layer<ToggleFilterInner, S>,
}

struct ToggleFilterInner {
    is_enabled: bool,
}

pub struct ToggleFilterHandle<S> {
    inner: tracing_subscriber::reload::Handle<ToggleFilterInner, S>,
}

impl<S> ToggleFilter<S>
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    pub fn new(is_enabled: bool) -> (ToggleFilter<S>, ToggleFilterHandle<S>) {
        let filter = ToggleFilterInner { is_enabled };
        let (filter, handle) = tracing_subscriber::reload::Layer::new(filter);
        let filter = ToggleFilter { inner: filter };
        let handle = ToggleFilterHandle { inner: handle };
        (filter, handle)
    }
}

impl<S> tracing_subscriber::layer::Filter<S> for ToggleFilter<S>
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    fn enabled(
        &self,
        metadata: &tracing_core::Metadata<'_>,
        cx: &tracing_subscriber::layer::Context<'_, S>,
    ) -> bool {
        self.inner.enabled(metadata, cx)
    }
}

impl<S> tracing_subscriber::layer::Filter<S> for ToggleFilterInner
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    fn enabled(
        &self,
        _metadata: &tracing_core::Metadata<'_>,
        _cx: &tracing_subscriber::layer::Context<'_, S>,
    ) -> bool {
        self.is_enabled
    }
}

impl<S> ToggleFilterHandle<S>
where
    S: tracing_core::Subscriber,
{
    pub fn enable(&mut self) -> Result<(), tracing_subscriber::reload::Error> {
        self.inner.reload(ToggleFilterInner { is_enabled: true })
    }

    pub fn disable(&mut self) -> Result<(), tracing_subscriber::reload::Error> {
        self.inner.reload(ToggleFilterInner { is_enabled: false })
    }

    pub fn toggle(&mut self) -> Result<(), tracing_subscriber::reload::Error> {
        self.inner.modify(|f| {
            f.is_enabled = !f.is_enabled;
        })
    }
}

pub struct ActionTraceToggle<S> {
    handle: Arc<Mutex<ToggleFilterHandle<S>>>,
    type_: ActionTraceToggleType,
}
#[derive(Debug, Clone)]
pub enum ActionTraceToggleType {
    Enable,
    Disable,
    Toggle,
}

impl<S> std::fmt::Debug for ActionTraceToggle<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Omit `handle`.
        f.debug_struct("ActionTraceToggle")
            .field("type_", &self.type_)
            .finish()
    }
}

impl<S> Clone for ActionTraceToggle<S> {
    fn clone(&self) -> Self {
        Self {
            handle: self.handle.clone(),
            type_: self.type_.clone(),
        }
    }
}

impl<S> ActionFnI for ActionTraceToggle<S>
where
    S: tracing_core::Subscriber,
{
    fn exec(&self, _state: &mut SabiniwmState) {
        match self.type_ {
            ActionTraceToggleType::Enable => self.handle.lock().unwrap().enable().unwrap(),
            ActionTraceToggleType::Disable => self.handle.lock().unwrap().disable().unwrap(),
            ActionTraceToggleType::Toggle => self.handle.lock().unwrap().toggle().unwrap(),
        }
    }
}

impl<S> ActionTraceToggle<S>
where
    S: tracing_core::Subscriber,
{
    pub fn new(handle: Arc<Mutex<ToggleFilterHandle<S>>>, type_: ActionTraceToggleType) -> Self {
        Self { handle, type_ }
    }
}
