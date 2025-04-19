use crate::state::SabiniwmState;
use smithay::desktop::{find_popup_root_surface, get_popup_toplevel_coords, PopupKind};
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::reexports::wayland_server::protocol::wl_seat;
use smithay::utils::Serial;
use smithay::wayland::shell::xdg::{
    PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
};

impl XdgShellHandler for SabiniwmState {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.inner.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        use smithay::reexports::wayland_server::Resource;

        let object_id = surface.wl_surface().id();
        let window = smithay::desktop::Window::new_wayland_window(surface);
        assert!(!self.inner.windows_waiting_mapping.contains_key(&object_id));
        self.inner
            .windows_waiting_mapping
            .insert(object_id, window.clone());
    }

    fn new_popup(&mut self, surface: PopupSurface, _positioner: PositionerState) {
        self.unconstrain_popup(&surface);
        let _ = self.inner.popups.track_popup(PopupKind::Xdg(surface));
    }

    fn reposition_request(
        &mut self,
        surface: PopupSurface,
        positioner: PositionerState,
        token: u32,
    ) {
        surface.with_pending_state(|state| {
            let geometry = positioner.get_geometry();
            state.geometry = geometry;
            state.positioner = positioner;
        });
        self.unconstrain_popup(&surface);
        surface.send_repositioned(token);
    }

    fn move_request(&mut self, _surface: ToplevelSurface, _seat: wl_seat::WlSeat, _serial: Serial) {
        // nop. Currently, moving windows by drag is not supproted.
    }

    fn resize_request(
        &mut self,
        _surface: ToplevelSurface,
        _seat: wl_seat::WlSeat,
        _serial: Serial,
        _edges: xdg_toplevel::ResizeEdge,
    ) {
        // nop. Currently, resizing windows by drag is not supproted.
    }

    fn grab(&mut self, _surface: PopupSurface, _seat: wl_seat::WlSeat, _serial: Serial) {
        // TODO popup grabs
    }
}

smithay::delegate_xdg_shell!(SabiniwmState);

impl SabiniwmState {
    fn unconstrain_popup(&self, popup: &PopupSurface) {
        let Ok(root) = find_popup_root_surface(&PopupKind::Xdg(popup.clone())) else {
            return;
        };
        let Some(window) = self
            .inner
            .space
            .elements()
            .find(|w| w.toplevel().unwrap().wl_surface() == &root)
        else {
            return;
        };

        let output = self.inner.space.outputs().next().unwrap();
        let output_geo = self.inner.space.output_geometry(output).unwrap();
        let window_geo = self.inner.space.element_geometry(window).unwrap();

        // The target geometry for the positioner should be relative to its parent's geometry, so
        // we will compute that here.
        let mut target = output_geo;
        target.loc -= get_popup_toplevel_coords(&PopupKind::Xdg(popup.clone()));
        target.loc -= window_geo.loc;

        popup.with_pending_state(|state| {
            state.geometry = state.positioner.get_unconstrained_geometry(target);
        });
    }
}
