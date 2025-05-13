use smithay::reexports::wayland_server::DisplayHandle;
use smithay::reexports::wayland_server::backend::GlobalId;

/// Call `DisplayHandle::remove_global::<State>()` on drop.
pub(crate) struct WlGlobal<State, Resource>
where
    State: 'static,
{
    global_id: GlobalId,
    display_handle: DisplayHandle,
    _state: std::marker::PhantomData<State>,
    _resource: std::marker::PhantomData<Resource>,
}

impl<State, Resource> Drop for WlGlobal<State, Resource> {
    fn drop(&mut self) {
        self.display_handle
            .remove_global::<State>(self.global_id.clone());
    }
}

impl<State, Resource> WlGlobal<State, Resource> {
    pub fn new(global_id: GlobalId, display_handle: DisplayHandle) -> Self {
        Self {
            global_id,
            display_handle,
            _state: std::marker::PhantomData,
            _resource: std::marker::PhantomData,
        }
    }
}
