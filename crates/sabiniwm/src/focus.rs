use crate::state::SabiniwmState;
use smithay::desktop::WindowSurface;
use smithay::input::Seat;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;

#[derive(derive_more::From, Debug, Clone, PartialEq)]
#[thin_delegate::register]
pub enum KeyboardFocusTarget {
    Window(smithay::desktop::Window),
    LayerSurface(smithay::desktop::LayerSurface),
    Popup(smithay::desktop::PopupKind),
    SessionLockSurface(WlSurface),
}

#[thin_delegate::fill_delegate(external_trait_def = crate::external_trait_def::smithay::utils)]
impl smithay::utils::IsAlive for KeyboardFocusTarget {}

#[thin_delegate::fill_delegate(
    external_trait_def = crate::external_trait_def::smithay::input::keyboard,
    scheme = |f| {
        match self {
            Self::Window(x) => match x.underlying_surface() {
                smithay::desktop::WindowSurface::Wayland(y) => f(y.wl_surface()),
                smithay::desktop::WindowSurface::X11(y) => f(y),
            }
            Self::LayerSurface(x) => f(x.wl_surface()),
            Self::Popup(x) => f(x.wl_surface()),
            Self::SessionLockSurface(x) => f(x),
        }
    }
)]
impl smithay::input::keyboard::KeyboardTarget<SabiniwmState> for KeyboardFocusTarget {}

impl smithay::wayland::seat::WaylandFocus for KeyboardFocusTarget {
    fn wl_surface(&self) -> Option<WlSurface> {
        match self {
            KeyboardFocusTarget::Window(x) => x.wl_surface(),
            KeyboardFocusTarget::LayerSurface(x) => Some(x.wl_surface().clone()),
            KeyboardFocusTarget::Popup(x) => Some(x.wl_surface().clone()),
            KeyboardFocusTarget::SessionLockSurface(x) => Some(x.clone()),
        }
    }
}

#[derive(derive_more::From, Debug, Clone, PartialEq)]
#[thin_delegate::register]
pub enum PointerFocusTarget {
    WlSurface(smithay::reexports::wayland_server::protocol::wl_surface::WlSurface),
    X11Surface(smithay::xwayland::X11Surface),
}

impl From<KeyboardFocusTarget> for PointerFocusTarget {
    fn from(x: KeyboardFocusTarget) -> Self {
        match x {
            KeyboardFocusTarget::Window(x) => match x.underlying_surface() {
                WindowSurface::Wayland(y) => PointerFocusTarget::from(y.wl_surface().clone()),
                WindowSurface::X11(y) => PointerFocusTarget::from(y.clone()),
            },
            KeyboardFocusTarget::LayerSurface(x) => {
                PointerFocusTarget::from(x.wl_surface().clone())
            }
            KeyboardFocusTarget::Popup(x) => PointerFocusTarget::from(x.wl_surface().clone()),
            KeyboardFocusTarget::SessionLockSurface(x) => PointerFocusTarget::from(x),
        }
    }
}

#[thin_delegate::fill_delegate(external_trait_def = crate::external_trait_def::smithay::utils)]
impl smithay::utils::IsAlive for PointerFocusTarget {}

#[thin_delegate::fill_delegate(external_trait_def = crate::external_trait_def::smithay::input::pointer)]
impl smithay::input::pointer::PointerTarget<SabiniwmState> for PointerFocusTarget {}

#[thin_delegate::fill_delegate(external_trait_def = crate::external_trait_def::smithay::input::touch)]
impl smithay::input::touch::TouchTarget<SabiniwmState> for PointerFocusTarget {}

#[thin_delegate::fill_delegate(external_trait_def = crate::external_trait_def::smithay::wayland::seet)]
impl smithay::wayland::seat::WaylandFocus for PointerFocusTarget {}
