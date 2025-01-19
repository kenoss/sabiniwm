use crate::focus::{KeyboardFocusTarget, PointerFocusTarget};
use crate::backend::{DmabufHandlerDelegate, BackendI};
use crate::state::{ClientState, SabiniwmState};
use smithay::desktop::space::SpaceElement;
use smithay::desktop::utils::surface_primary_scanout_output;
use smithay::desktop::{PopupKind, PopupManager};
use smithay::input::keyboard::LedState;
use smithay::input::pointer::{CursorImageStatus, PointerHandle};
use smithay::input::{Seat, SeatHandler, SeatState};
use smithay::reexports::wayland_protocols::xdg::decoration::zv1::server::
    zxdg_toplevel_decoration_v1::Mode as DecorationMode;
use smithay::reexports::wayland_protocols::xdg::decoration::{self as xdg_decoration};
use smithay::reexports::wayland_server::protocol::wl_data_source::WlDataSource;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::Resource;
use smithay::utils::Rectangle;
use smithay::wayland::compositor::{get_parent, with_states};
use smithay::wayland::fractional_scale::{with_fractional_scale, FractionalScaleHandler};
use smithay::wayland::input_method::{InputMethodHandler, PopupSurface};
use smithay::wayland::keyboard_shortcuts_inhibit::{
    KeyboardShortcutsInhibitHandler, KeyboardShortcutsInhibitState, KeyboardShortcutsInhibitor,
};
use smithay::wayland::output::OutputHandler;
use smithay::wayland::pointer_constraints::{with_pointer_constraint, PointerConstraintsHandler};
use smithay::wayland::seat::WaylandFocus;
use smithay::wayland::security_context::{
    SecurityContext, SecurityContextHandler, SecurityContextListenerSource,
};
use smithay::wayland::selection::data_device::{
    set_data_device_focus, ClientDndGrabHandler, DataDeviceHandler, DataDeviceState,
    ServerDndGrabHandler,
};
use smithay::wayland::selection::primary_selection::{
    set_primary_focus, PrimarySelectionHandler, PrimarySelectionState,
};
use smithay::wayland::selection::wlr_data_control::{DataControlHandler, DataControlState};
use smithay::wayland::selection::{SelectionHandler, SelectionSource, SelectionTarget};
use smithay::wayland::shell::xdg::decoration::XdgDecorationHandler;
use smithay::wayland::shell::xdg::{ToplevelSurface, XdgToplevelSurfaceData};
use smithay::wayland::shm::{ShmHandler, ShmState};
use smithay::wayland::xdg_activation::{
    XdgActivationHandler, XdgActivationState, XdgActivationToken, XdgActivationTokenData,
};
use smithay::wayland::xdg_foreign::{XdgForeignHandler, XdgForeignState};
use smithay::wayland::xwayland_keyboard_grab::XWaylandKeyboardGrabHandler;
use std::os::unix::io::OwnedFd;
use std::sync::Arc;

smithay::delegate_compositor!(SabiniwmState);

impl DataDeviceHandler for SabiniwmState {
    fn data_device_state(&self) -> &DataDeviceState {
        &self.inner.data_device_state
    }
}

impl ClientDndGrabHandler for SabiniwmState {
    fn started(
        &mut self,
        _source: Option<WlDataSource>,
        icon: Option<WlSurface>,
        _seat: Seat<Self>,
    ) {
        self.inner.dnd_icon = icon;
    }
    fn dropped(&mut self, _seat: Seat<Self>) {
        self.inner.dnd_icon = None;
    }
}

impl ServerDndGrabHandler for SabiniwmState {
    fn send(&mut self, _mime_type: String, _fd: OwnedFd, _seat: Seat<Self>) {
        unreachable!("server-side grabs are not supported");
    }
}

smithay::delegate_data_device!(SabiniwmState);

impl OutputHandler for SabiniwmState {}

smithay::delegate_output!(SabiniwmState);

impl SelectionHandler for SabiniwmState {
    type SelectionUserData = ();

    fn new_selection(
        &mut self,
        ty: SelectionTarget,
        source: Option<SelectionSource>,
        _seat: Seat<Self>,
    ) {
        if let Some(xwm) = self.inner.xwm.as_mut() {
            if let Err(err) = xwm.new_selection(ty, source.map(|source| source.mime_types())) {
                warn!(?err, ?ty, "Failed to set Xwayland selection");
            }
        }
    }

    fn send_selection(
        &mut self,
        ty: SelectionTarget,
        mime_type: String,
        fd: OwnedFd,
        _seat: Seat<Self>,
        _user_data: &(),
    ) {
        if let Some(xwm) = self.inner.xwm.as_mut() {
            if let Err(err) = xwm.send_selection(ty, mime_type, fd, self.inner.loop_handle.clone())
            {
                warn!(?err, "Failed to send primary (X11 -> Wayland)");
            }
        }
    }
}

impl PrimarySelectionHandler for SabiniwmState {
    fn primary_selection_state(&self) -> &PrimarySelectionState {
        &self.inner.primary_selection_state
    }
}

smithay::delegate_primary_selection!(SabiniwmState);

impl DataControlHandler for SabiniwmState {
    fn data_control_state(&self) -> &DataControlState {
        &self.inner.data_control_state
    }
}

smithay::delegate_data_control!(SabiniwmState);

impl ShmHandler for SabiniwmState {
    fn shm_state(&self) -> &ShmState {
        &self.inner.shm_state
    }
}

smithay::delegate_shm!(SabiniwmState);

impl SeatHandler for SabiniwmState {
    type KeyboardFocus = KeyboardFocusTarget;
    type PointerFocus = PointerFocusTarget;
    type TouchFocus = PointerFocusTarget;

    fn seat_state(&mut self) -> &mut SeatState<SabiniwmState> {
        &mut self.inner.seat_state
    }

    fn focus_changed(&mut self, seat: &Seat<Self>, target: Option<&KeyboardFocusTarget>) {
        let dh = &self.inner.display_handle;

        let wl_surface = target.and_then(WaylandFocus::wl_surface);

        let focus = wl_surface.and_then(|s| dh.get_client(s.id()).ok());
        set_data_device_focus(dh, seat, focus.clone());
        set_primary_focus(dh, seat, focus);
    }
    fn cursor_image(&mut self, _seat: &Seat<Self>, image: CursorImageStatus) {
        *self.inner.cursor_status.lock().unwrap() = image;
    }

    fn led_state_changed(&mut self, _seat: &Seat<Self>, led_state: LedState) {
        self.backend.update_led_state(led_state)
    }
}

smithay::delegate_seat!(SabiniwmState);
smithay::delegate_text_input_manager!(SabiniwmState);

mod tablet_seat_handler {
    use super::*;
    use smithay::backend::input::TabletToolDescriptor;
    use smithay::wayland::tablet_manager::TabletSeatHandler;

    impl TabletSeatHandler for SabiniwmState {
        fn tablet_tool_image(&mut self, _tool: &TabletToolDescriptor, image: CursorImageStatus) {
            // TODO: tablet tools should have their own cursors
            *self.inner.cursor_status.lock().unwrap() = image;
        }
    }

    smithay::delegate_tablet_manager!(SabiniwmState);
}

impl InputMethodHandler for SabiniwmState {
    fn new_popup(&mut self, surface: PopupSurface) {
        if let Err(err) = self.inner.popups.track_popup(PopupKind::from(surface)) {
            warn!("Failed to track popup: {}", err);
        }
    }

    fn dismiss_popup(&mut self, surface: PopupSurface) {
        if let Some(parent) = surface.get_parent().map(|parent| parent.surface.clone()) {
            let _ = PopupManager::dismiss_popup(&parent, &PopupKind::from(surface));
        }
    }

    fn parent_geometry(&self, parent: &WlSurface) -> Rectangle<i32, smithay::utils::Logical> {
        self.inner
            .space
            .elements()
            .find_map(|window| {
                (window.smithay_window().wl_surface().as_ref() == Some(parent))
                    .then(|| window.geometry())
            })
            .unwrap_or_default()
    }
}

smithay::delegate_input_method_manager!(SabiniwmState);

impl KeyboardShortcutsInhibitHandler for SabiniwmState {
    fn keyboard_shortcuts_inhibit_state(&mut self) -> &mut KeyboardShortcutsInhibitState {
        &mut self.inner.keyboard_shortcuts_inhibit_state
    }

    fn new_inhibitor(&mut self, inhibitor: KeyboardShortcutsInhibitor) {
        // Just grant the wish for everyone
        inhibitor.activate();
    }
}

smithay::delegate_keyboard_shortcuts_inhibit!(SabiniwmState);
smithay::delegate_virtual_keyboard_manager!(SabiniwmState);
smithay::delegate_pointer_gestures!(SabiniwmState);
smithay::delegate_relative_pointer!(SabiniwmState);

impl PointerConstraintsHandler for SabiniwmState {
    fn new_constraint(&mut self, surface: &WlSurface, pointer: &PointerHandle<Self>) {
        // XXX region
        if pointer
            .current_focus()
            .and_then(|x| x.wl_surface())
            .as_ref()
            == Some(surface)
        {
            with_pointer_constraint(surface, pointer, |constraint| {
                constraint.unwrap().activate();
            });
        }
    }
}

smithay::delegate_pointer_constraints!(SabiniwmState);
smithay::delegate_viewporter!(SabiniwmState);

impl XdgActivationHandler for SabiniwmState {
    fn activation_state(&mut self) -> &mut XdgActivationState {
        &mut self.inner.xdg_activation_state
    }

    fn token_created(&mut self, _token: XdgActivationToken, data: XdgActivationTokenData) -> bool {
        if let Some((serial, seat)) = data.serial {
            let keyboard = self.inner.seat.get_keyboard().unwrap();
            Seat::from_resource(&seat) == Some(self.inner.seat.clone())
                && keyboard
                    .last_enter()
                    .map(|last_enter| serial.is_no_older_than(&last_enter))
                    .unwrap_or(false)
        } else {
            false
        }
    }

    fn request_activation(
        &mut self,
        _token: XdgActivationToken,
        token_data: XdgActivationTokenData,
        surface: WlSurface,
    ) {
        if token_data.timestamp.elapsed().as_secs() < 10 {
            // Just grant the wish
            let w = self
                .inner
                .space
                .elements()
                .find(|window| {
                    window
                        .smithay_window()
                        .wl_surface()
                        .map(|s| s == surface)
                        .unwrap_or(false)
                })
                .cloned();
            if let Some(window) = w {
                self.inner.space.raise_element(&window, true);
            }
        }
    }
}

smithay::delegate_xdg_activation!(SabiniwmState);

impl XdgDecorationHandler for SabiniwmState {
    fn new_decoration(&mut self, toplevel: ToplevelSurface) {
        use xdg_decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode;
        // Set the default to client side
        toplevel.with_pending_state(|state| {
            state.decoration_mode = Some(Mode::ClientSide);
        });
    }
    fn request_mode(&mut self, toplevel: ToplevelSurface, mode: DecorationMode) {
        use xdg_decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode;

        toplevel.with_pending_state(|state| {
            state.decoration_mode = Some(match mode {
                DecorationMode::ServerSide => Mode::ServerSide,
                _ => Mode::ClientSide,
            });
        });

        let initial_configure_sent = with_states(toplevel.wl_surface(), |states| {
            states
                .data_map
                .get::<XdgToplevelSurfaceData>()
                .unwrap()
                .lock()
                .unwrap()
                .initial_configure_sent
        });
        if initial_configure_sent {
            toplevel.send_pending_configure();
        }
    }
    fn unset_mode(&mut self, toplevel: ToplevelSurface) {
        use xdg_decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode;
        toplevel.with_pending_state(|state| {
            state.decoration_mode = Some(Mode::ClientSide);
        });
        let initial_configure_sent = with_states(toplevel.wl_surface(), |states| {
            states
                .data_map
                .get::<XdgToplevelSurfaceData>()
                .unwrap()
                .lock()
                .unwrap()
                .initial_configure_sent
        });
        if initial_configure_sent {
            toplevel.send_pending_configure();
        }
    }
}

smithay::delegate_xdg_decoration!(SabiniwmState);
smithay::delegate_layer_shell!(SabiniwmState);
smithay::delegate_presentation!(SabiniwmState);

impl FractionalScaleHandler for SabiniwmState {
    fn new_fractional_scale(
        &mut self,
        surface: smithay::reexports::wayland_server::protocol::wl_surface::WlSurface,
    ) {
        // Here we can set the initial fractional scale
        //
        // First we look if the surface already has a primary scan-out output, if not
        // we test if the surface is a subsurface and try to use the primary scan-out output
        // of the root surface. If the root also has no primary scan-out output we just try
        // to use the first output of the toplevel.
        // If the surface is the root we also try to use the first output of the toplevel.
        //
        // If all the above tests do not lead to a output we just use the first output
        // of the space (which in case of this compositor will also be the output a toplevel will
        // initially be placed on)
        #[allow(clippy::redundant_clone)]
        let mut root = surface.clone();
        while let Some(parent) = get_parent(&root) {
            root = parent;
        }

        with_states(&surface, |states| {
            let primary_scanout_output = surface_primary_scanout_output(&surface, states)
                .or_else(|| {
                    if root != surface {
                        with_states(&root, |states| {
                            surface_primary_scanout_output(&root, states).or_else(|| {
                                self.window_for_surface(&root).and_then(|window| {
                                    self.inner
                                        .space
                                        .outputs_for_element(&window)
                                        .first()
                                        .cloned()
                                })
                            })
                        })
                    } else {
                        self.window_for_surface(&root).and_then(|window| {
                            self.inner
                                .space
                                .outputs_for_element(&window)
                                .first()
                                .cloned()
                        })
                    }
                })
                .or_else(|| self.inner.space.outputs().next().cloned());
            if let Some(output) = primary_scanout_output {
                with_fractional_scale(states, |fractional_scale| {
                    fractional_scale.set_preferred_scale(output.current_scale().fractional_scale());
                });
            }
        });
    }
}

smithay::delegate_fractional_scale!(SabiniwmState);

impl SecurityContextHandler for SabiniwmState {
    fn context_created(
        &mut self,
        source: SecurityContextListenerSource,
        security_context: SecurityContext,
    ) {
        self.inner
            .loop_handle
            .insert_source(source, move |client_stream, _, state| {
                let client_state = ClientState {
                    security_context: Some(security_context.clone()),
                    ..ClientState::default()
                };
                if let Err(err) = state
                    .inner
                    .display_handle
                    .insert_client(client_stream, Arc::new(client_state))
                {
                    warn!("Error adding wayland client: {}", err);
                };
            })
            .expect("Failed to init wayland socket source");
    }
}

smithay::delegate_security_context!(SabiniwmState);

impl XWaylandKeyboardGrabHandler for SabiniwmState {
    fn keyboard_focus_for_xsurface(&self, surface: &WlSurface) -> Option<KeyboardFocusTarget> {
        let window = self
            .inner
            .space
            .elements()
            .find(|window| window.smithay_window().wl_surface().as_ref() == Some(surface))?;
        Some(KeyboardFocusTarget::Window(window.smithay_window().clone()))
    }
}

smithay::delegate_xwayland_keyboard_grab!(SabiniwmState);

impl XdgForeignHandler for SabiniwmState {
    fn xdg_foreign_state(&mut self) -> &mut XdgForeignState {
        &mut self.inner.xdg_foreign_state
    }
}

smithay::delegate_xdg_foreign!(SabiniwmState);

impl smithay::wayland::dmabuf::DmabufHandler for SabiniwmState {
    fn dmabuf_state(&mut self) -> &mut smithay::wayland::dmabuf::DmabufState {
        self.backend.dmabuf_state()
    }

    fn dmabuf_imported(
        &mut self,
        global: &smithay::wayland::dmabuf::DmabufGlobal,
        dmabuf: smithay::backend::allocator::dmabuf::Dmabuf,
        notifier: smithay::wayland::dmabuf::ImportNotifier,
    ) {
        if self.backend.dmabuf_imported(global, dmabuf) {
            let _ = notifier.successful::<SabiniwmState>();
        } else {
            notifier.failed();
        }
    }
}

smithay::delegate_dmabuf!(SabiniwmState);
