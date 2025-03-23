use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::reexports::calloop::{LoopHandle, RegistrationToken};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

/// Flexiblly reschedulable timer, without reregistering.
///
/// By default, it calls the given callback just once when it is started. If one calls
/// `FlexibleTimerController::schedule_next()`, it will do that again. One can consider this as
/// repeated timer callback with variable duration defaulting to pause.
///
/// Comparison:
///
/// - `calloop::timer::Timer`: It allows rescheduling only in the callback.
/// - `calloop::transient::Transient<Timer>`: It needs reregistering.
///
/// It is used to construct `RenderLoop` that cares VBlank.
struct FlexibleTimerController<State> {
    loop_handle: LoopHandle<'static, State>,
    /// A timer calls `Self::callback()` (outer callback), which calls `inner_callback`.
    #[allow(clippy::type_complexity)]
    inner_callback: Rc<Box<dyn Fn(&mut State)>>,
    /// A struct shared with outer callback.
    timer_state: Rc<RefCell<FlexibleTimerState>>,
    /// `Some` iff a timer is registered.
    registration_token: Option<RegistrationToken>,
}

struct FlexibleTimerState {
    is_running: bool,
    committed: ScheduleInfo,
    /// `Some` iff the current thread is in the `inner_callback`.
    pending: Option<ScheduleInfo>,
}

struct ScheduleInfo {
    deadline: Option<Instant>,
}

#[derive(Debug, thiserror::Error)]
enum FlexibleTimerControllerStartError {
    #[error("already running")]
    AlreadyRunning,
}

#[derive(Debug, thiserror::Error)]
enum FlexibleTimerControllerScheduleError {
    #[error("not running")]
    NotRunning,
}

impl<State> FlexibleTimerController<State>
where
    State: 'static,
{
    pub fn new<F>(loop_handle: LoopHandle<'static, State>, callback: F) -> Self
    where
        F: Fn(&mut State) + 'static,
    {
        let timer_state = FlexibleTimerState {
            is_running: false,
            committed: ScheduleInfo { deadline: None },
            pending: None,
        };
        let timer_state = Rc::new(RefCell::new(timer_state));
        Self {
            loop_handle,
            inner_callback: Rc::new(Box::new(callback)),
            timer_state,
            registration_token: None,
        }
    }

    #[inline]
    #[allow(clippy::type_complexity)]
    fn callback(
        timer_state: &Rc<RefCell<FlexibleTimerState>>,
        inner_callback: &Rc<Box<dyn Fn(&mut State)>>,
        state: &mut State,
    ) -> TimeoutAction {
        {
            let mut timer_state = timer_state.borrow_mut();

            if !timer_state.is_running {
                return TimeoutAction::Drop;
            }

            timer_state.pending = Some(ScheduleInfo { deadline: None });
        }

        inner_callback(state);

        {
            let mut timer_state = timer_state.borrow_mut();

            if !timer_state.is_running {
                return TimeoutAction::Drop;
            }

            timer_state.committed = timer_state.pending.take().unwrap();

            if let Some(deadline) = timer_state.committed.deadline {
                TimeoutAction::ToInstant(deadline)
            } else {
                TimeoutAction::Drop
            }
        }
    }

    /// Starts a loop.
    ///
    /// Returns an error if it is already running. Callers can ignore it if it doesn't care about it.
    pub fn start(&mut self) -> Result<(), FlexibleTimerControllerStartError> {
        {
            let mut timer_state = self.timer_state.borrow_mut();

            if timer_state.is_running {
                return Err(FlexibleTimerControllerStartError::AlreadyRunning);
            }

            timer_state.is_running = true;
        }

        assert!(self.timer_state.borrow().committed.deadline.is_none());
        assert!(self.timer_state.borrow().pending.is_none());
        assert!(self.registration_token.is_none());

        self.schedule_next_aux(Instant::now());

        Ok(())
    }

    /// Stops the loop.
    ///
    /// Note that it does nothnig and quietly returns if it is not running.
    pub fn stop(&mut self) {
        {
            let mut timer_state = self.timer_state.borrow_mut();

            if !timer_state.is_running {
                return;
            }

            timer_state.is_running = false;
            timer_state.committed.deadline = None;
            timer_state.pending = None;
        }

        if let Some(registration_token) = self.registration_token.take() {
            self.loop_handle.remove(registration_token);
        }
    }

    /// Schedules next callback to be called at `deadline`.
    ///
    /// Returns an error if it is not running.
    ///
    /// If it has already scheduled one, it updates that.
    pub fn schedule_next(
        &mut self,
        deadline: Instant,
    ) -> Result<(), FlexibleTimerControllerScheduleError> {
        if !self.timer_state.borrow().is_running {
            return Err(FlexibleTimerControllerScheduleError::NotRunning);
        }

        {
            let mut timer_state = self.timer_state.borrow_mut();
            // If it is called in the `inner_callback`.
            if let Some(pending) = timer_state.pending.as_mut() {
                pending.deadline = Some(deadline);
                return Ok(());
            }
        }

        if let Some(registration_token) = self.registration_token.take() {
            self.loop_handle.remove(registration_token);
        }

        self.schedule_next_aux(deadline);

        Ok(())
    }

    fn schedule_next_aux(&mut self, deadline: Instant) {
        assert!(self.registration_token.is_none());

        {
            let mut timer_state = self.timer_state.borrow_mut();
            assert!(timer_state.is_running);
            timer_state.committed.deadline = Some(deadline);
            assert!(timer_state.pending.is_none());
        }

        // TODO: Update self.timer_state.committed?

        let timer = Timer::from_deadline(deadline);
        let timer_state = self.timer_state.clone();
        let inner_callback = self.inner_callback.clone();
        let registration_token = self.loop_handle
            .insert_source(timer, move |_, _, state| {
                Self::callback(&timer_state, &inner_callback, state)
            })
            .unwrap(/* safety: Registration of `Timer` never fails. */);
        self.registration_token = Some(registration_token);
    }
}

/// Repeated timer callback with variable duration for render loop.
///
/// This is built on top of `FlexibleTimerController`.
pub(crate) struct RenderLoop<State> {
    timer: FlexibleTimerController<State>,
    /// Unit: 0.001Hz. E.g. about 60000 for 60Hz.
    refresh_rate: u32,
}

impl<State> RenderLoop<State>
where
    State: 'static,
{
    pub fn new<F>(
        loop_handle: LoopHandle<'static, State>,
        output: &smithay::output::Output,
        callback: F,
    ) -> Self
    where
        F: Fn(&mut State) + 'static,
    {
        let refresh_rate: u32 = output
            .current_mode()
            .map(|mode| mode.refresh)
            .unwrap_or(60_000)
            .try_into()
            .unwrap(/* Refresh rate is positive. */);
        // The unit is 0.001Hz. Check the value is in 0.5Hz -- 500Hz.
        assert!(500 < refresh_rate && refresh_rate < 500_000);

        let timer = FlexibleTimerController::new(loop_handle, callback);

        Self {
            timer,
            refresh_rate,
        }
    }

    #[cfg(test)]
    pub fn new_for_test<F>(
        loop_handle: LoopHandle<'static, State>,
        refresh_rate: u32,
        callback: F,
    ) -> Self
    where
        F: Fn(&mut State) + 'static,
    {
        let timer = FlexibleTimerController::new(loop_handle, callback);

        Self {
            timer,
            refresh_rate,
        }
    }

    pub fn start(&mut self) {
        self.timer.start().unwrap();
    }

    pub fn stop(&mut self) {
        self.timer.stop();
    }

    pub fn schedule_now(&mut self) {
        let _ = self.timer.schedule_next(Instant::now());
    }

    pub fn on_render_frame(&mut self, should_schedule_render: bool) {
        if !should_schedule_render {
            return;
        }

        // If scanout is not done, continue the loop.
        //
        // TODO: Pause the loop if no need to render.

        let deadline = self.next_deadline();
        let _ = self.timer.schedule_next(deadline);
    }

    pub fn on_vblank(&mut self) {
        let deadline = self.next_deadline();
        let _ = self.timer.schedule_next(deadline);
    }

    fn next_deadline(&self) -> Instant {
        // TODO:
        //
        // - Subtract a duration for sabiniwm's render so that we can submit a next frame before
        //   VSync. See also
        //   https://github.com/Smithay/smithay/blob/8e49b9bb1849f0ead1ba2c7cd76802fc12ad6ac3/anvil/src/udev.rs#L1305
        // - Use `last_render_ended_at` for base point.
        let duration =
            Duration::from_micros((1_000_000f32 * 1000.0 / self.refresh_rate as f32) as u64);
        Instant::now()
            .checked_add(duration)
            .expect("std::time::Instant doesn't overflow")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use smithay::reexports::calloop::{EventLoop, LoopSignal};

    #[test]
    fn smoke_test() {
        struct TestState {
            render_loop: RenderLoop<TestState>,
            loop_handle: LoopHandle<'static, TestState>,
            loop_signal: LoopSignal,
            n: usize,
        }

        let mut event_loop = EventLoop::try_new().unwrap();

        let mut render_loop =
            RenderLoop::new_for_test(event_loop.handle(), 60_000, |state: &mut TestState| {
                if state.n % 3 == 0 {
                    state.render_loop.on_render_frame(false);

                    let timer = Timer::from_duration(Duration::from_millis(16));
                    state
                        .loop_handle
                        .insert_source(timer, |_, _, state| {
                            state.render_loop.on_vblank();
                            TimeoutAction::Drop
                        })
                        .unwrap();
                } else {
                    state.render_loop.on_render_frame(true);
                }

                state.n -= 1;
                if state.n == 0 {
                    state.loop_signal.stop();
                }
            });
        render_loop.start();

        let mut state = TestState {
            render_loop,
            loop_handle: event_loop.handle(),
            loop_signal: event_loop.get_signal(),
            n: 10,
        };

        event_loop.run(None, &mut state, |_| {}).unwrap();
    }
}
