#![warn(rust_2018_idioms)]
// If no backend is enabled, a large portion of the codebase is unused.
// So silence this useless warning for the CI.
#![cfg_attr(
    not(any(feature = "winit", feature = "udev")),
    allow(dead_code, unused_imports)
)]

#[allow(unused_imports)]
#[macro_use]
extern crate tracing;

pub mod cursor;
pub mod drawing;
pub mod focus;
pub mod input_handler;
pub mod render;
pub mod shell;
pub mod state;
pub mod state_delegate;
#[cfg(feature = "udev")]
pub mod udev;
#[cfg(feature = "winit")]
pub mod winit;

pub use state::{CalloopData, ClientState, SabiniwmState};