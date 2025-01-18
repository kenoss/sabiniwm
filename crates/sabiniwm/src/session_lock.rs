use crate::smithay_ext::{OutputExt, SizeExt};
use crate::state::SabiniwmState;
use smithay::backend::renderer::element::solid::SolidColorBuffer;
use smithay::reexports::wayland_protocols::ext::session_lock::v1::server::ext_session_lock_v1::ExtSessionLockV1;
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::wayland::session_lock::{
    LockSurface, SessionLockHandler, SessionLockManagerState, SessionLocker,
};
use std::collections::HashMap;

pub(crate) enum SessionLockState<T> {
    NotLocked,
    Locked(T),
}

pub(crate) struct SessionLockData {
    // `LockedButClientGone` is not used for this field as we don't have callback that notifies that the client has gone.
    // Read access is only allowed via `SessionLockData::normalized_state()`.
    state: SessionLockState<ExtSessionLockV1>,
    manager_state: SessionLockManagerState,
    output_assocs: HashMap<smithay::output::Output, SessionLockOutputAssoc>,
}

pub(crate) struct SessionLockOutputAssoc {
    pub lock_surface: Option<LockSurface>,
    pub background: SolidColorBuffer,
}

#[cfg(not(feature = "debug_session_lock_client_dead"))]
const LOCKED_BACKGROUND_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
// Make it transparent when a feature flag for debug is enabled.
#[cfg(feature = "debug_session_lock_client_dead")]
const LOCKED_BACKGROUND_COLOR: [f32; 4] = [0.0, 0.1, 0.0, 0.1];

impl SessionLockData {
    pub fn new(display_handle: &smithay::reexports::wayland_server::DisplayHandle) -> Self {
        let manager_state =
            SessionLockManagerState::new::<SabiniwmState, _>(display_handle, |_| true);
        Self {
            state: SessionLockState::NotLocked,
            manager_state,
            output_assocs: HashMap::new(),
        }
    }

    pub fn on_output_added(&mut self, output: &smithay::output::Output) {
        let size = output.current_logical_size();
        let output_assoc = SessionLockOutputAssoc {
            lock_surface: None,
            background: SolidColorBuffer::new(size, LOCKED_BACKGROUND_COLOR),
        };
        self.output_assocs.insert(output.clone(), output_assoc);
    }

    pub fn is_locked(&self) -> bool {
        match &self.state {
            SessionLockState::NotLocked => false,
            SessionLockState::Locked(_) => true,
        }
    }

    pub fn get_lock_surface(
        &self,
        output: &smithay::output::Output,
    ) -> SessionLockState<&SessionLockOutputAssoc> {
        match &self.state {
            SessionLockState::NotLocked => SessionLockState::NotLocked,
            SessionLockState::Locked(_) => {
                SessionLockState::Locked(self.output_assocs.get(output).unwrap())
            }
        }
    }
}

impl SessionLockHandler for SessionLockData {
    fn lock_state(&mut self) -> &mut SessionLockManagerState {
        &mut self.manager_state
    }

    fn lock(&mut self, confirmation: SessionLocker) {
        match &self.state {
            SessionLockState::NotLocked => {}
            SessionLockState::Locked(_) => {
                info!("new session lock request is refused as already locked with another client");
                return;
            }
        }

        self.state = SessionLockState::Locked(confirmation.ext_session_lock().clone());
        confirmation.lock();
    }

    fn unlock(&mut self) {
        self.state = SessionLockState::NotLocked;
        for output_assoc in self.output_assocs.values_mut() {
            output_assoc.lock_surface = None;
        }
    }

    fn new_surface(&mut self, surface: LockSurface, wl_output: WlOutput) {
        use smithay::output::Output;
        use smithay::wayland::compositor::{send_surface_state, with_states};

        let Some(output) = Output::from_resource(&wl_output) else {
            warn!("Output not found for WlOutput: wl_output = {:?}", wl_output);
            return;
        };

        surface.with_pending_state(|states| {
            let size = output.current_logical_size().to_u32().unwrap(/* size is positive */);
            states.size = Some(size);
        });
        let scale = output.current_scale().integer_scale();
        let transform = output.current_transform();
        let wl_surface = surface.wl_surface();
        with_states(wl_surface, |data| {
            send_surface_state(wl_surface, data, scale, transform);
        });
        surface.send_configure();

        let output_assoc = self.output_assocs.get_mut(&output).unwrap(/* SessionLockOutputAssoc exists if Output exists */);
        output_assoc.lock_surface = Some(surface);

        debug!("new lock surface added: output = {:?}", output);
    }
}

impl SessionLockHandler for SabiniwmState {
    fn lock_state(&mut self) -> &mut SessionLockManagerState {
        self.inner.session_lock_data.lock_state()
    }

    fn lock(&mut self, confirmation: SessionLocker) {
        self.inner.session_lock_data.lock(confirmation)
    }

    fn unlock(&mut self) {
        self.inner.session_lock_data.unlock();

        self.update_focus_when_session_lock_changed();
    }

    fn new_surface(&mut self, surface: LockSurface, wl_output: WlOutput) {
        self.inner.session_lock_data.new_surface(surface, wl_output);

        self.update_focus_when_session_lock_changed();
    }
}

smithay::delegate_session_lock!(SabiniwmState);
