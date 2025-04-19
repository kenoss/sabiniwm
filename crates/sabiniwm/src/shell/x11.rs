use crate::focus::KeyboardFocusTarget;
use crate::state::SabiniwmState;
use smithay::utils::{Logical, Rectangle};
use smithay::wayland::selection::data_device::{
    clear_data_device_selection, current_data_device_selection_userdata,
    request_data_device_client_selection, set_data_device_selection,
};
use smithay::wayland::selection::primary_selection::{
    clear_primary_selection, current_primary_selection_userdata, request_primary_client_selection,
    set_primary_selection,
};
use smithay::wayland::selection::SelectionTarget;
use smithay::wayland::xwayland_shell::{XWaylandShellHandler, XWaylandShellState};
use smithay::xwayland::xwm::{Reorder, ResizeEdge as X11ResizeEdge, XwmId};
use smithay::xwayland::{X11Surface, X11Wm, XwmHandler};
use std::os::unix::io::OwnedFd;

impl XWaylandShellHandler for SabiniwmState {
    fn xwayland_shell_state(&mut self) -> &mut XWaylandShellState {
        &mut self.inner.xwayland_shell_state
    }
}

smithay::delegate_xwayland_shell!(SabiniwmState);

impl XwmHandler for SabiniwmState {
    fn xwm_state(&mut self, _xwm: XwmId) -> &mut X11Wm {
        self.inner.xwm.as_mut().unwrap()
    }

    fn new_window(&mut self, _xwm: XwmId, _window: X11Surface) {}
    fn new_override_redirect_window(&mut self, _xwm: XwmId, _window: X11Surface) {}

    fn map_window_request(&mut self, _xwm: XwmId, window: X11Surface) {
        window.set_mapped(true).unwrap();

        let window = smithay::desktop::Window::new_x11_window(window);
        let window_id = self.inner.view.register_window(window);
        self.inner.view.run_manage_hook(
            &self.inner.config_delegate,
            window_id,
            self.inner.display_handle.clone(),
        );
        self.inner.view.layout(&mut self.inner.space);
        self.reflect_focus_from_stackset();
    }

    fn mapped_override_redirect_window(&mut self, _xwm: XwmId, window: X11Surface) {
        let window = smithay::desktop::Window::new_x11_window(window);
        let window_id = self.inner.view.register_window(window);
        self.inner.view.set_focus(window_id);
        self.inner.view.layout(&mut self.inner.space);
        self.reflect_focus_from_stackset();
    }

    fn unmapped_window(&mut self, _xwm: XwmId, window: X11Surface) {
        let maybe = self
            .inner
            .space
            .elements()
            .find(|e| matches!(e.smithay_window().x11_surface(), Some(w) if w == &window))
            .cloned();
        if let Some(elem) = maybe {
            self.inner.space.unmap_elem(&elem)
        }
        if !window.is_override_redirect() {
            window.set_mapped(false).unwrap();
        }
    }

    fn destroyed_window(&mut self, _xwm: XwmId, _window: X11Surface) {}

    fn configure_request(
        &mut self,
        _xwm: XwmId,
        window: X11Surface,
        _x: Option<i32>,
        _y: Option<i32>,
        w: Option<u32>,
        h: Option<u32>,
        _reorder: Option<Reorder>,
    ) {
        // we just set the new size, but don't let windows move themselves around freely
        let mut geo = window.geometry();
        if let Some(w) = w {
            geo.size.w = w as i32;
        }
        if let Some(h) = h {
            geo.size.h = h as i32;
        }
        let _ = window.configure(geo);
    }

    fn configure_notify(
        &mut self,
        _xwm: XwmId,
        window: X11Surface,
        geometry: Rectangle<i32, Logical>,
        _above: Option<u32>,
    ) {
        let Some(elem) = self
            .inner
            .space
            .elements()
            .find(|e| matches!(e.smithay_window().x11_surface(), Some(w) if w == &window))
            .cloned()
        else {
            return;
        };
        self.inner.space.map_element(elem, geometry.loc, false);
        // TODO: We don't properly handle the order of override-redirect windows here,
        //       they are always mapped top and then never reordered.
    }

    fn move_request(&mut self, _xwm: XwmId, _window: X11Surface, _button: u32) {
        // nop. Currently, moving windows by drag is not supproted.
    }

    fn resize_request(
        &mut self,
        _xwm: XwmId,
        _window: X11Surface,
        _button: u32,
        _edges: X11ResizeEdge,
    ) {
        // nop. Currently, resizing windows by drag is not supproted.
    }

    fn allow_selection_access(&mut self, xwm: XwmId, _selection: SelectionTarget) -> bool {
        if let Some(keyboard) = self.inner.seat.get_keyboard() {
            // check that an X11 window is focused
            if let Some(KeyboardFocusTarget::Window(w)) = keyboard.current_focus() {
                if let Some(surface) = w.x11_surface() {
                    if surface.xwm_id().unwrap() == xwm {
                        return true;
                    }
                }
            }
        }
        false
    }

    fn send_selection(
        &mut self,
        _xwm: XwmId,
        selection: SelectionTarget,
        mime_type: String,
        fd: OwnedFd,
    ) {
        match selection {
            SelectionTarget::Clipboard => {
                if let Err(err) =
                    request_data_device_client_selection(&self.inner.seat, mime_type, fd)
                {
                    error!(
                        ?err,
                        "Failed to request current wayland clipboard for Xwayland",
                    );
                }
            }
            SelectionTarget::Primary => {
                if let Err(err) = request_primary_client_selection(&self.inner.seat, mime_type, fd)
                {
                    error!(
                        ?err,
                        "Failed to request current wayland primary selection for Xwayland",
                    );
                }
            }
        }
    }

    fn new_selection(&mut self, _xwm: XwmId, selection: SelectionTarget, mime_types: Vec<String>) {
        trace!(?selection, ?mime_types, "Got Selection from X11",);
        // TODO check, that focused windows is X11 window before doing this
        match selection {
            SelectionTarget::Clipboard => set_data_device_selection(
                &self.inner.display_handle,
                &self.inner.seat,
                mime_types,
                (),
            ),
            SelectionTarget::Primary => {
                set_primary_selection(&self.inner.display_handle, &self.inner.seat, mime_types, ())
            }
        }
    }

    fn cleared_selection(&mut self, _xwm: XwmId, selection: SelectionTarget) {
        match selection {
            SelectionTarget::Clipboard => {
                if current_data_device_selection_userdata(&self.inner.seat).is_some() {
                    clear_data_device_selection(&self.inner.display_handle, &self.inner.seat)
                }
            }
            SelectionTarget::Primary => {
                if current_primary_selection_userdata(&self.inner.seat).is_some() {
                    clear_primary_selection(&self.inner.display_handle, &self.inner.seat)
                }
            }
        }
    }
}
