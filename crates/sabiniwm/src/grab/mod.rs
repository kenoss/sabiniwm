mod window_move_grab;

use crate::state::SabiniwmState;
use window_move_grab::WindowMoveGrab;

impl SabiniwmState {
    pub(crate) fn grab_window_for_move(
        &mut self,
        serial: smithay::utils::Serial,
        target: crate::focus::PointerFocusTarget,
        event: &smithay::input::pointer::ButtonEvent,
        release_condition: Box<
            dyn Fn(&smithay::input::pointer::ButtonEvent) -> bool + Send + 'static,
        >,
    ) {
        use smithay::input::pointer::{Focus, GrabStartData};
        use smithay::wayland::seat::WaylandFocus;

        let pointer = self.inner.seat.get_pointer().unwrap();

        if pointer.is_grabbed() {
            return;
        }

        let surface = target.wl_surface();
        let Some(window) = self
            .inner
            .space
            .elements()
            .find(|window| window.smithay_window().wl_surface() == surface)
        else {
            return;
        };

        let start_data = GrabStartData {
            focus: pointer
                .current_focus()
                .map(|x| (x, pointer.current_location())),
            button: event.button,
            location: pointer.current_location(),
        };
        let grab = WindowMoveGrab {
            release_condition,
            start_data,
            window_id: window.id(),
            grabbed_loc: pointer.current_location() - window.geometry_actual().loc.to_f64(),
        };

        self.inner.view.make_window_float(window.id());
        self.inner.view.layout(&mut self.inner.space);

        pointer.set_grab(self, grab, serial, Focus::Clear);
    }
}
