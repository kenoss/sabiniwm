//! # sabiniwm: A tiling Wayland compositor, influenced by xmonad
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
pub mod config;
mod const_;
pub mod cursor;
mod envvar;
mod external_trait_def;
pub mod focus;
mod grab;
pub mod input;
pub(crate) mod input_event;
pub mod input_handler;
pub mod model;
pub mod pointer;
pub mod render;
pub(crate) mod render_loop;
pub(crate) mod session_lock;
pub mod shell;
pub(crate) mod smithay_ext;
pub mod state;
pub mod state_delegate;
#[allow(unused)]
pub(crate) mod util;
pub mod view;
pub(crate) mod wl_global;

pub mod reexports {
    pub use smithay;
}

pub use state::{ClientState, SabiniwmState};
