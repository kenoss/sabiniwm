//! # sabiniwm: A tiling Wayland compositor, influenced xmonad
//!
//! Not documented yet. Wait for v0.1.0.

#[allow(unused_imports)]
#[macro_use]
extern crate tracing;

#[allow(unused_imports)]
#[macro_use]
extern crate maplit;

pub mod action;
pub mod backend;
pub mod cursor;
mod envvar;
mod external_trait_def;
pub mod focus;
pub mod input;
pub(crate) mod input_event;
pub mod input_handler;
pub(crate) mod model;
pub mod pointer;
pub mod render;
pub(crate) mod render_loop;
pub mod shell;
pub mod state;
pub mod state_delegate;
#[allow(unused)]
pub(crate) mod util;
pub mod view;
pub(crate) mod wl_global;

pub use state::{ClientState, SabiniwmState};
