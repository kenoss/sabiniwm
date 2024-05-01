use crate::input::keymap::KeymapEntry;
use crate::input::KeySeq;
use crate::state::Sabiniwm;
use crate::util::Id;
use crate::view::window::Window;
use smithay::backend::input::{
    AbsolutePositionEvent, Axis, AxisSource, ButtonState, Event, InputBackend, InputEvent,
    KeyState, KeyboardKeyEvent, PointerAxisEvent, PointerButtonEvent,
};
use smithay::input::keyboard::FilterResult;
use smithay::input::pointer::{AxisFrame, ButtonEvent, MotionEvent};
use smithay::utils::{Logical, Point, Serial, SERIAL_COUNTER};

impl Sabiniwm {
    pub(crate) fn process_input_event<I: InputBackend>(&mut self, event: InputEvent<I>) {
        let should_update_focus =
            self.focus_update_decider
                .should_update_focus(&self.seat, &self.space, &event);

        match event {
            InputEvent::Keyboard { event, .. } => {
                let serial = SERIAL_COUNTER.next_serial();

                let time = Event::time_msec(&event);

                // Note that `Seat::get_keyboard()` locks a field. If we call `Sabiniwm::process_action()` in the `filter` (the
                // last argument), it will deadlock (if it hits a path calling e.g. `Seat::get_keyborad()` in it).
                let action = self.seat.get_keyboard().unwrap().input(
                    self,
                    event.key_code(),
                    event.state(),
                    // Note that this `serial` will not be used for `KeybordHandler::input_forward()` if
                    // `KeyboardHandler::input_intercept()` returned `FilterResult::Intercept`. So, issuing a new `Serial` in
                    // `Sabiniwm::process_action` is OK.
                    serial,
                    time,
                    |this, _, keysym_handle| match event.state() {
                        KeyState::Pressed => {
                            let was_empty = this.keyseq.is_empty();
                            for key in KeySeq::extract(&keysym_handle).into_vec() {
                                this.keyseq.push(key);
                                match this.keymap.get(&this.keyseq).clone() {
                                    KeymapEntry::Complete(action) => {
                                        this.keyseq.clear();
                                        return FilterResult::Intercept(Some(action));
                                    }
                                    KeymapEntry::Incomplete => {}
                                    KeymapEntry::None => {
                                        this.keyseq.clear();
                                        if was_empty {
                                            return FilterResult::Forward;
                                        } else {
                                            return FilterResult::Intercept(None);
                                        }
                                    }
                                }
                            }
                            FilterResult::Intercept(None)
                        }
                        KeyState::Released => {
                            if this.keyseq.is_empty() {
                                FilterResult::Forward
                            } else {
                                FilterResult::Intercept(None)
                            }
                        }
                    },
                );
                if let Some(action) = action.flatten() {
                    self.process_action(&action);
                }
            }
            InputEvent::PointerMotion { .. } => {}
            InputEvent::PointerMotionAbsolute { event, .. } => {
                let serial = SERIAL_COUNTER.next_serial();

                let pointer = self.seat.get_pointer().unwrap();

                let output = self.space.outputs().next().unwrap();
                let output_geo = self.space.output_geometry(output).unwrap();
                let pos = event.position_transformed(output_geo.size) + output_geo.loc.to_f64();
                let under = self.surface_under(pos);

                if should_update_focus {
                    self.update_focus(serial, pos);
                }

                pointer.motion(
                    self,
                    under,
                    &MotionEvent {
                        serial,
                        time: event.time_msec(),
                        location: pos,
                    },
                );
                pointer.frame(self);
            }
            InputEvent::PointerButton { event, .. } => {
                let serial = SERIAL_COUNTER.next_serial();

                let pointer = self.seat.get_pointer().unwrap();

                let button = event.button_code();
                let button_state = event.state();

                if should_update_focus {
                    self.update_focus(serial, pointer.current_location());
                }

                pointer.button(
                    self,
                    &ButtonEvent {
                        serial,
                        time: event.time_msec(),
                        button,
                        state: button_state,
                    },
                );
                pointer.frame(self);
            }
            InputEvent::PointerAxis { event, .. } => {
                let source = event.source();

                let horizontal_amount = event.amount(Axis::Horizontal).unwrap_or_else(|| {
                    event.amount_v120(Axis::Horizontal).unwrap_or(0.0) * 3.0 / 120.
                });
                let vertical_amount = event.amount(Axis::Vertical).unwrap_or_else(|| {
                    event.amount_v120(Axis::Vertical).unwrap_or(0.0) * 3.0 / 120.
                });
                let horizontal_amount_discrete = event.amount_v120(Axis::Horizontal);
                let vertical_amount_discrete = event.amount_v120(Axis::Vertical);

                let mut frame = AxisFrame::new(event.time_msec()).source(source);
                if horizontal_amount != 0.0 {
                    frame = frame.value(Axis::Horizontal, horizontal_amount);
                    if let Some(discrete) = horizontal_amount_discrete {
                        frame = frame.v120(Axis::Horizontal, discrete as i32);
                    }
                }
                if vertical_amount != 0.0 {
                    frame = frame.value(Axis::Vertical, vertical_amount);
                    if let Some(discrete) = vertical_amount_discrete {
                        frame = frame.v120(Axis::Vertical, discrete as i32);
                    }
                }

                if source == AxisSource::Finger {
                    if event.amount(Axis::Horizontal) == Some(0.0) {
                        frame = frame.stop(Axis::Horizontal);
                    }
                    if event.amount(Axis::Vertical) == Some(0.0) {
                        frame = frame.stop(Axis::Vertical);
                    }
                }

                let pointer = self.seat.get_pointer().unwrap();
                pointer.axis(self, frame);
                pointer.frame(self);
            }
            _ => {}
        }
    }

    fn update_focus(&mut self, serial: Serial, pos: Point<f64, Logical>) {
        let Some(window) = self.space.element_under(pos).map(|(w, _)| w).cloned() else {
            return;
        };

        self.view.set_focus(window.id());
        self.reflect_focus_from_stackset(Some(serial));
    }

    pub(crate) fn reflect_focus_from_stackset(&mut self, serial: Option<Serial>) {
        let Some(window) = self.view.focused_window() else {
            return;
        };

        self.space.raise_element(window, true);

        // TODO: Check whether this is necessary.
        for window in self.space.elements() {
            window.toplevel().unwrap().send_pending_configure();
        }

        let serial = serial.unwrap_or_else(|| SERIAL_COUNTER.next_serial());

        let keyboard = self.seat.get_keyboard().unwrap();
        keyboard.set_focus(
            self,
            Some(window.toplevel().unwrap().wl_surface().clone()),
            serial,
        );
    }
}

// Focus follows mouse.
//
// Prevents updating focus due to too high sensitivity of touchpad.
//
// TODO: Stabilize interface and make it public for configuration.
pub(crate) struct FocusUpdateDecider {
    last_window_id: Option<Id<Window>>,
    last_pos: Point<f64, Logical>,
}

impl FocusUpdateDecider {
    const DISTANCE_THRESHOLD: f64 = 16.0;

    pub fn new() -> Self {
        Self {
            last_window_id: None,
            last_pos: Point::default(),
        }
    }

    fn should_update_focus<I>(
        &mut self,
        seat: &smithay::input::Seat<Sabiniwm>,
        space: &smithay::desktop::Space<Window>,
        event: &InputEvent<I>,
    ) -> bool
    where
        I: InputBackend,
    {
        fn center_of_pixel(pos: Point<f64, Logical>) -> Point<f64, Logical> {
            (pos.x.floor() + 0.5, pos.y.floor() + 0.5).into()
        }

        match event {
            InputEvent::PointerMotionAbsolute { event } => {
                // Requirements:
                //
                // - Focus should be updated when mouse enters to another window.
                // - Focus should not be updated if a non mouse event updated focus last time, e.g. spawning a new window, and
                //   the mouse is not sufficiently moved.

                let output = space.outputs().next().unwrap();
                let output_geo = space.output_geometry(output).unwrap();
                let pos = event.position_transformed(output_geo.size) + output_geo.loc.to_f64();
                let under_window_id = space.element_under(pos).map(|(w, _)| w.id());
                let d = pos - self.last_pos;
                let distance = (d.x * d.x + d.y * d.y).sqrt();

                let ret =
                    self.last_window_id != under_window_id || distance > Self::DISTANCE_THRESHOLD;
                if ret {
                    self.last_window_id = under_window_id;
                    self.last_pos = center_of_pixel(pos);
                }
                ret
            }
            InputEvent::PointerButton { event } => {
                let pointer = seat.get_pointer().unwrap();

                let button_state = event.state();

                !pointer.is_grabbed() && button_state == ButtonState::Pressed
            }
            _ => false,
        }
    }
}
