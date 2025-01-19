use crate::config::{ConfigDelegate, ConfigDelegateUnstableI};
use crate::input::keymap::KeymapEntry;
use crate::input::KeySeq;
use crate::state::SabiniwmState;
use crate::util::Id;
use crate::view::window::Window;
use smithay::backend::input::{
    AbsolutePositionEvent, Axis, AxisSource, ButtonState, Event, InputBackend, InputEvent,
    KeyState, KeyboardKeyEvent, PointerAxisEvent, PointerButtonEvent,
};
use smithay::input::keyboard::FilterResult;
use smithay::input::pointer::{AxisFrame, ButtonEvent, MotionEvent};
use smithay::utils::{Logical, Point, Serial, SERIAL_COUNTER};
use std::ops::ControlFlow;

impl SabiniwmState {
    pub(crate) fn process_input_event<I: InputBackend>(&mut self, event: InputEvent<I>) {
        let serial = SERIAL_COUNTER.next_serial();

        let should_update_focus = self.inner.focus_update_decider.should_update_focus(
            &self.inner.config_delegate,
            &self.inner.seat,
            &self.inner.space,
            Timing::BeforeProcessEvent,
            &event,
        );
        if should_update_focus {
            self.update_focus(serial);
        }

        match &event {
            InputEvent::DeviceAdded { .. } | InputEvent::DeviceRemoved { .. } => {
                // Handled in backend layer.
                unreachable!();
            }
            InputEvent::Keyboard { event } => {
                let time = Event::time_msec(event);

                // Note that `Seat::get_keyboard()` locks a field. If we call `SabiniwmState::process_action()` in the `filter` (the
                // last argument), it will deadlock (if it hits a path calling e.g. `Seat::get_keyborad()` in it).
                let action = self.inner.seat.get_keyboard().unwrap().input(
                    self,
                    event.key_code(),
                    event.state(),
                    // Note that this `serial` will not be used for `KeybordHandler::input_forward()` if
                    // `KeyboardHandler::input_intercept()` returned `FilterResult::Intercept`. So, issuing a new `Serial` in
                    // `SabiniwmState::process_action` is OK.
                    serial,
                    time,
                    |this, _, keysym_handle| match event.state() {
                        KeyState::Pressed => {
                            let was_empty = this.inner.keyseq.is_empty();
                            for key in KeySeq::extract(&keysym_handle).into_vec() {
                                this.inner.keyseq.push(key);
                                debug!("{:?}", this.inner.keyseq);
                                match this.inner.keymap.get(&this.inner.keyseq).clone() {
                                    KeymapEntry::Complete(action) => {
                                        this.inner.keyseq.clear();
                                        return FilterResult::Intercept(Some(action));
                                    }
                                    KeymapEntry::Incomplete => {}
                                    KeymapEntry::None => {
                                        this.inner.keyseq.clear();
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
                            if this.inner.keyseq.is_empty() {
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
            InputEvent::PointerMotion { event } => {
                use smithay::backend::input::PointerMotionEvent;
                use smithay::input::pointer::RelativeMotionEvent;

                let pointer = self.inner.seat.get_pointer().unwrap();

                trace!(
                    "InputEvent::PointerMotion: current location before = {:?}",
                    pointer.current_location()
                );

                let output = self.inner.space.outputs().next().unwrap();
                let output_rect = self.inner.space.output_geometry(output).unwrap().to_f64();
                let loc = (pointer.current_location() + event.delta()).constrain(output_rect);
                let under = self.surface_under(loc);

                pointer.motion(
                    self,
                    under.clone(),
                    &MotionEvent {
                        serial,
                        time: event.time_msec(),
                        location: loc,
                    },
                );
                pointer.relative_motion(
                    self,
                    under,
                    &RelativeMotionEvent {
                        utime: event.time(),
                        delta: event.delta(),
                        delta_unaccel: event.delta_unaccel(),
                    },
                );
                pointer.frame(self);

                trace!(
                    "InputEvent::PointerMotion: current location after = {:?}",
                    pointer.current_location()
                );
            }
            InputEvent::PointerMotionAbsolute { event } => {
                let pointer = self.inner.seat.get_pointer().unwrap();

                let output = self.inner.space.outputs().next().unwrap();
                let output_geo = self.inner.space.output_geometry(output).unwrap();
                let pos = event.position_transformed(output_geo.size) + output_geo.loc.to_f64();
                let under = self.surface_under(pos);

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
            InputEvent::PointerButton { event } => {
                let pointer = self.inner.seat.get_pointer().unwrap();

                // Update pointer focus.
                //
                // Consider the case that `PointerButton` event is emitted just after workspace focus is changed and a workspace
                // is shown.
                //
                // - A window A was under the pointer in the previous workspace and a window B is under the pointer in the current.
                // - A window A was under the pointer in the previous workspace and no window is under the pointer in the current.
                // - No window was under the pointer in the previous workspace and a window B is under the pointer in the current.
                // - No window was under the pointer in the previous workspace and no window is under the pointer in the current.
                //
                // In each case, the event will be derivered to the A/none instead of B/none if we don't update focus here.
                //
                // To prevent this, we call `PointerHandle::motion()` to update focus. In the above case, it changes
                // `smithay::input::pointer::PointerInnerHandle::focus` and calls `crate::PointerFocusTarget::replace()`.
                //
                // It is legitimate to unconditionally calls it (i.e. in the other case): pointer related events should be
                // derivered to a target that is under the pointer at the event timing.
                let pos = pointer.current_location();
                let under = self.surface_under(pos);
                pointer.motion(
                    self,
                    under,
                    &MotionEvent {
                        serial,
                        time: event.time_msec(),
                        location: pos,
                    },
                );

                let button = event.button_code();
                let button_state = event.state();
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
            InputEvent::PointerAxis { event } => {
                let source = event.source();

                let horizontal_amount = event.amount(Axis::Horizontal).unwrap_or_else(|| {
                    event.amount_v120(Axis::Horizontal).unwrap_or(0.0) * 15.0 / 120.
                });
                let vertical_amount = event.amount(Axis::Vertical).unwrap_or_else(|| {
                    event.amount_v120(Axis::Vertical).unwrap_or(0.0) * 15.0 / 120.
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

                let pointer = self.inner.seat.get_pointer().unwrap();
                pointer.axis(self, frame);
                pointer.frame(self);
            }
            InputEvent::SwitchToggle { event } => {
                use smithay::backend::input::{Switch, SwitchState, SwitchToggleEvent};

                match (event.switch(), event.state()) {
                    (Some(Switch::Lid), SwitchState::On) => {
                        self.inner.config_delegate.on_lid_closed();
                    }
                    (Some(Switch::Lid), SwitchState::Off) => {
                        self.inner.config_delegate.on_lid_opened();
                    }
                    _ => {}
                }
            }
            _ => {}
        }

        let should_update_focus = self.inner.focus_update_decider.should_update_focus(
            &self.inner.config_delegate,
            &self.inner.seat,
            &self.inner.space,
            Timing::AfterProcessEvent,
            &event,
        );
        if should_update_focus {
            self.update_focus(serial);
        }
    }

    fn reset_focus_if_session_is_locked(
        &mut self,
        serial: Serial,
        pos: Point<f64, Logical>,
    ) -> std::ops::ControlFlow<(), ()> {
        let keyboard = self.inner.seat.get_keyboard().unwrap();

        let Some(output) = self.inner.space.outputs().find(|o| {
            let geometry = self.inner.space.output_geometry(o).unwrap();
            geometry.contains(pos.to_i32_round())
        }) else {
            return ControlFlow::Break(());
        };

        use crate::session_lock::SessionLockState;
        match self.inner.session_lock_data.get_lock_surface(output) {
            SessionLockState::NotLocked => ControlFlow::Continue(()),
            SessionLockState::Locked(output_assoc) => {
                match &output_assoc.lock_surface {
                    Some(lock_surface) => {
                        use crate::focus::KeyboardFocusTarget;

                        keyboard.set_focus(
                            self,
                            Some(KeyboardFocusTarget::SessionLockSurface(
                                lock_surface.wl_surface().clone(),
                            )),
                            serial,
                        );
                        ControlFlow::Break(())
                    }
                    // Make sure to focus out to prevent emitting events to apps except the lock client
                    // even if a lock surface doesn't exist.
                    None => {
                        keyboard.set_focus(self, None, serial);
                        ControlFlow::Break(())
                    }
                }
            }
        }
    }

    // TODO: Use `pub(in crate::session_lock)` instead. (It causes an compilation error.)
    pub(crate) fn update_focus_when_session_lock_changed(&mut self) {
        let serial = SERIAL_COUNTER.next_serial();
        self.update_focus(serial);
    }

    fn update_focus(&mut self, serial: Serial) {
        let pointer = self.inner.seat.get_pointer().unwrap();
        let pos = pointer.current_location();

        match self.reset_focus_if_session_is_locked(serial, pos) {
            ControlFlow::Continue(_) => {}
            ControlFlow::Break(_) => return,
        }

        let Some(window) = self.inner.space.element_under(pos).map(|(w, _)| w).cloned() else {
            return;
        };

        self.inner.view.set_focus(window.id());
        self.reflect_focus_from_stackset_aux(serial);
    }

    pub(crate) fn reflect_focus_from_stackset(&mut self) {
        let serial = SERIAL_COUNTER.next_serial();
        let pointer = self.inner.seat.get_pointer().unwrap();
        let pos = pointer.current_location();

        match self.reset_focus_if_session_is_locked(serial, pos) {
            ControlFlow::Continue(_) => {}
            ControlFlow::Break(_) => return,
        }

        self.reflect_focus_from_stackset_aux(serial);
    }

    pub(crate) fn reflect_focus_from_stackset_aux(&mut self, serial: Serial) {
        let Some(window) = self.inner.view.focused_window() else {
            return;
        };

        self.inner.space.raise_element(window, true);

        // TODO: Check whether this is necessary.
        for window in self.inner.space.elements() {
            if let Some(toplevel) = window.toplevel() {
                toplevel.send_pending_configure();
            }
        }

        let keyboard = self.inner.seat.get_keyboard().unwrap();
        keyboard.set_focus(self, Some(window.smithay_window().clone().into()), serial);
    }
}

/// Focus follows mouse.
///
/// Prevents updating focus due to too high sensitivity of touchpad.
pub(crate) struct FocusUpdateDecider {
    last_window_id: Option<Id<Window>>,
    last_pos: Point<f64, Logical>,
}

#[derive(Debug)]
enum Timing {
    BeforeProcessEvent,
    AfterProcessEvent,
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
        config_delegate: &ConfigDelegate,
        seat: &smithay::input::Seat<SabiniwmState>,
        space: &smithay::desktop::Space<Window>,
        timing: Timing,
        event: &InputEvent<I>,
    ) -> bool
    where
        I: InputBackend,
    {
        fn center_of_pixel(pos: Point<f64, Logical>) -> Point<f64, Logical> {
            (pos.x.floor() + 0.5, pos.y.floor() + 0.5).into()
        }

        match (timing, event) {
            (Timing::BeforeProcessEvent, InputEvent::PointerButton { event }) => {
                let pointer = seat.get_pointer().unwrap();
                let button_state = event.state();

                !pointer.is_grabbed() && button_state == ButtonState::Pressed
            }
            (
                Timing::AfterProcessEvent,
                InputEvent::PointerMotion { .. } | InputEvent::PointerMotionAbsolute { .. },
            ) => {
                if !config_delegate.focus_follows_mouse() {
                    return false;
                }

                // Requirements:
                //
                // - Focus should be updated when mouse enters to another window.
                // - Focus should not be updated if a non mouse event updated focus last time, e.g.
                //   spawning a new window, and the mouse is not sufficiently moved.

                let pointer = seat.get_pointer().unwrap();
                let pos = pointer.current_location();
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
            _ => false,
        }
    }
}
