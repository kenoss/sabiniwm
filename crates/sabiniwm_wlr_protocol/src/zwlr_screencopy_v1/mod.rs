//! <https://wayland.app/protocols/wlr-screencopy-unstable-v1>
//!
//! To use this module,
//!
//! - Add [`ScreencopyState`] to your `State` to hold a global.
//! - Implement [`ScreencopyHandler`] for your `State`.
//! - Implement rendering to [`ScreencopyFrameRequest`].
//! - Delegate with [`sabiniwm_wlr_protocol::delegate_zwlr_screencopy_v1!`].
//!
//! An implementation needs to handle `on_frame_request()`. It is expected to attach it to `Output`,
//! and then take it and render a frame to it.
//!
//! ```rust
//! # struct State { screencopy_state: sabiniwm_wlr_protocol::zwlr_screencopy_v1::ScreencopyState }
//!
//! use smithay::reexports::wayland_server::protocol::wl_shm;
//!
//! impl sabiniwm_wlr_protocol::zwlr_screencopy_v1::ScreencopyHandler for State {
//!     const TRANSFORM_BEHAVIOR: sabiniwm_wlr_protocol::zwlr_screencopy_v1::TransformBehavior =
//!         sabiniwm_wlr_protocol::zwlr_screencopy_v1::TransformBehavior::ScaleActualTerminal;
//!
//!     const SUPPORTED_FORMATS: sabiniwm_wlr_protocol::zwlr_screencopy_v1::SupportedFormats =
//!         sabiniwm_wlr_protocol::zwlr_screencopy_v1::SupportedFormats {
//!             shm_format: wl_shm::Format::Argb8888,
//!             dmabuf_formats: &[
//!                 smithay::backend::allocator::Fourcc::Abgr8888,
//!                 smithay::backend::allocator::Fourcc::Argb8888,
//!                 smithay::backend::allocator::Fourcc::Xbgr8888,
//!                 smithay::backend::allocator::Fourcc::Xrgb8888,
//!             ],
//!         };
//!
//!     fn on_frame_request(
//!         &self,
//!         frame_request: sabiniwm_wlr_protocol::zwlr_screencopy_v1::ScreencopyFrameRequest,
//!         mut output: smithay::output::Output,
//!         should_wait_damage: bool,
//!     ) {
//!         use sabiniwm_wlr_protocol::zwlr_screencopy_v1::OutputExtForScreencopy;
//!
//!         // Attach `ScreencopyFrameRequest` to `Output`. This will be handled by
//!         // `OutputExtForScreencopy::take_screencopy_frame_requests()` when the next render.
//!         output.add_screencopy_frame_request(frame_request);
//!
//!         if (!should_wait_damage) {
//!           // Render now.
//!         }
//!     }
//! }
//!
//! sabiniwm_wlr_protocol::delegate_zwlr_screencopy_v1!(State);
//! ```

mod global;
mod internal;
mod output_ext_for_screencopy;
mod zwlr_screencopy_frame_v1;
mod zwlr_screencopy_manager_v1;

use crate as sabiniwm_wlr_protocol;
pub use crate::zwlr_screencopy_v1::zwlr_screencopy_frame_v1::ScreencopyFrameRequest;
use smithay::reexports::wayland_server::protocol::wl_shm;

mod hidden {
    pub use crate::zwlr_screencopy_v1::global::{ScreencopyGlobalData, ScreencopyState};
    pub use crate::zwlr_screencopy_v1::output_ext_for_screencopy::OutputExtForScreencopy;
    pub use crate::zwlr_screencopy_v1::zwlr_screencopy_frame_v1::ScreencopyFrameData;
    pub use crate::zwlr_screencopy_v1::zwlr_screencopy_manager_v1::ScreencopyManagerData;
}

#[doc(hidden)]
pub use hidden::*;

/// Controls behavior of `Transform` of screencopy.
#[derive(Debug, Clone, Copy)]
pub enum TransformBehavior {
    /// Doesn't use scale nor transforms.
    NoTransform,
    /// Use only scale.
    Scale,
    /// Use scale, and actual transform.
    ScaleActual,
    /// Use scale, actual transform, and terminal transform.
    ScaleActualTerminal,
}

/// Supported formats of buffer for screencopy
pub struct SupportedFormats {
    pub shm_format: wl_shm::Format,
    pub dmabuf_formats: &'static [smithay::backend::allocator::Fourcc],
}

pub trait ScreencopyHandler {
    const TRANSFORM_BEHAVIOR: sabiniwm_wlr_protocol::zwlr_screencopy_v1::TransformBehavior;

    const SUPPORTED_FORMATS: sabiniwm_wlr_protocol::zwlr_screencopy_v1::SupportedFormats;

    /// Called when we get a frame request
    fn on_frame_request(
        &self,
        frame: sabiniwm_wlr_protocol::zwlr_screencopy_v1::ScreencopyFrameRequest,
        output: smithay::output::Output,
        should_wait_damage: bool,
    );
}

/// Code holder for `Dispatch`
///
/// Crate internal data. Public only for `delegate_zwlr_screencopy_v1!`.
///
/// Never constructed.
// TODO(https://github.com/rust-lang/rust/issues/35121): Use never type.
#[doc(hidden)]
pub struct ScreencopyDispatcher;

/// Delegate `Dispatch` for the argument to `zwlr_screencopy_v1` module.
#[macro_export]
macro_rules! delegate_zwlr_screencopy_v1 {
    ($(@<$( $lt:tt $( : $clt:tt $(+ $dlt:tt )* )? ),+>)? $ty: ty) => {
        smithay::reexports::wayland_server::delegate_global_dispatch!($(@< $( $lt $( : $clt $(+ $dlt )* )? ),+ >)? $ty: [
            smithay::reexports::wayland_protocols_wlr::screencopy::v1::server::zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1: $crate::zwlr_screencopy_v1::ScreencopyGlobalData
        ] => $crate::zwlr_screencopy_v1::ScreencopyDispatcher);

        smithay::reexports::wayland_server::delegate_dispatch!($(@< $( $lt $( : $clt $(+ $dlt )* )? ),+ >)? $ty: [
            smithay::reexports::wayland_protocols_wlr::screencopy::v1::server::zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1: $crate::zwlr_screencopy_v1::ScreencopyManagerData
        ] => $crate::zwlr_screencopy_v1::ScreencopyDispatcher);

        smithay::reexports::wayland_server::delegate_dispatch!($(@< $( $lt $( : $clt $(+ $dlt )* )? ),+ >)? $ty: [
            smithay::reexports::wayland_protocols_wlr::screencopy::v1::server::zwlr_screencopy_frame_v1::ZwlrScreencopyFrameV1: $crate::zwlr_screencopy_v1::ScreencopyFrameData
        ] => $crate::zwlr_screencopy_v1::ScreencopyDispatcher);
    };
}
