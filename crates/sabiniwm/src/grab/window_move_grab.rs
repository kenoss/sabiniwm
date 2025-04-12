use crate::focus::PointerFocusTarget;
use crate::state::SabiniwmState;
use crate::util::Id;
use crate::view::window::Window;
use smithay::utils::{Logical, Point};

pub(super) struct WindowMoveGrab {
    pub release_condition:
        Box<dyn Fn(&smithay::input::pointer::ButtonEvent) -> bool + Send + 'static>,
    pub start_data: smithay::input::pointer::GrabStartData<SabiniwmState>,
    pub window_id: Id<Window>,
    // Delta of pointer location from left top of window.
    pub grabbed_loc: Point<f64, Logical>,
}

mod pointer_grab {
    use super::*;
    use smithay::input::pointer::{
        AxisFrame, ButtonEvent, GestureHoldBeginEvent, GestureHoldEndEvent, GesturePinchBeginEvent,
        GesturePinchEndEvent, GesturePinchUpdateEvent, GestureSwipeBeginEvent,
        GestureSwipeEndEvent, GestureSwipeUpdateEvent, GrabStartData, MotionEvent,
        PointerInnerHandle, RelativeMotionEvent,
    };

    impl smithay::input::pointer::PointerGrab<SabiniwmState> for WindowMoveGrab {
        fn motion(
            &mut self,
            state: &mut SabiniwmState,
            handle: &mut PointerInnerHandle<'_, SabiniwmState>,
            _focus: Option<(PointerFocusTarget, Point<f64, Logical>)>,
            event: &MotionEvent,
        ) {
            handle.motion(state, None, event);

            let loc = (event.location - self.grabbed_loc).to_i32_round();
            state
                .inner
                .view
                .update_float_window_with(self.window_id, |fw| {
                    fw.geometry.loc = loc;
                });
            state.inner.view.layout(&mut state.inner.space);
        }

        fn relative_motion(
            &mut self,
            state: &mut SabiniwmState,
            handle: &mut PointerInnerHandle<'_, SabiniwmState>,
            focus: Option<(PointerFocusTarget, Point<f64, Logical>)>,
            event: &RelativeMotionEvent,
        ) {
            handle.relative_motion(state, focus, event);
        }

        fn button(
            &mut self,
            state: &mut SabiniwmState,
            handle: &mut PointerInnerHandle<'_, SabiniwmState>,
            event: &ButtonEvent,
        ) {
            if (self.release_condition)(event) {
                handle.unset_grab(self, state, event.serial, event.time, true);
            } else {
                handle.button(state, event);
            }
        }

        fn axis(
            &mut self,
            state: &mut SabiniwmState,
            handle: &mut PointerInnerHandle<'_, SabiniwmState>,
            details: AxisFrame,
        ) {
            handle.axis(state, details)
        }

        fn frame(
            &mut self,
            state: &mut SabiniwmState,
            handle: &mut PointerInnerHandle<'_, SabiniwmState>,
        ) {
            handle.frame(state);
        }

        fn gesture_swipe_begin(
            &mut self,
            state: &mut SabiniwmState,
            handle: &mut PointerInnerHandle<'_, SabiniwmState>,
            event: &GestureSwipeBeginEvent,
        ) {
            handle.gesture_swipe_begin(state, event);
        }

        fn gesture_swipe_update(
            &mut self,
            state: &mut SabiniwmState,
            handle: &mut PointerInnerHandle<'_, SabiniwmState>,
            event: &GestureSwipeUpdateEvent,
        ) {
            handle.gesture_swipe_update(state, event);
        }

        fn gesture_swipe_end(
            &mut self,
            state: &mut SabiniwmState,
            handle: &mut PointerInnerHandle<'_, SabiniwmState>,
            event: &GestureSwipeEndEvent,
        ) {
            handle.gesture_swipe_end(state, event);
        }

        fn gesture_pinch_begin(
            &mut self,
            state: &mut SabiniwmState,
            handle: &mut PointerInnerHandle<'_, SabiniwmState>,
            event: &GesturePinchBeginEvent,
        ) {
            handle.gesture_pinch_begin(state, event);
        }

        fn gesture_pinch_update(
            &mut self,
            state: &mut SabiniwmState,
            handle: &mut PointerInnerHandle<'_, SabiniwmState>,
            event: &GesturePinchUpdateEvent,
        ) {
            handle.gesture_pinch_update(state, event);
        }

        fn gesture_pinch_end(
            &mut self,
            state: &mut SabiniwmState,
            handle: &mut PointerInnerHandle<'_, SabiniwmState>,
            event: &GesturePinchEndEvent,
        ) {
            handle.gesture_pinch_end(state, event);
        }

        fn gesture_hold_begin(
            &mut self,
            state: &mut SabiniwmState,
            handle: &mut PointerInnerHandle<'_, SabiniwmState>,
            event: &GestureHoldBeginEvent,
        ) {
            handle.gesture_hold_begin(state, event);
        }

        fn gesture_hold_end(
            &mut self,
            state: &mut SabiniwmState,
            handle: &mut PointerInnerHandle<'_, SabiniwmState>,
            event: &GestureHoldEndEvent,
        ) {
            handle.gesture_hold_end(state, event);
        }

        fn start_data(&self) -> &GrabStartData<SabiniwmState> {
            &self.start_data
        }

        fn unset(&mut self, _data: &mut SabiniwmState) {}
    }
}
