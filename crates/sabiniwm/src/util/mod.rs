pub(crate) mod console;
mod focused_vec;
mod id;
pub(crate) mod panic;

pub use focused_vec::*;
pub use id::*;

/// Utility trait to decouple `EventLoop::insert_source()` and handling logic.
pub(crate) trait EventHandler<Event> {
    fn handle_event(&mut self, event: Event);
}
