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
}

#[thin_delegate::fill_delegate(external_trait_def = crate::external_trait_def::smithay::utils)]
impl smithay::utils::IsAlive for KeyboardFocusTarget {}

#[thin_delegate::fill_delegate(
    external_trait_def = crate::external_trait_def::smithay::input::keyboard,
    scheme = |f| {
        match self {
            Self::Window(w) => match w.underlying_surface() {
                smithay::desktop::WindowSurface::Wayland(s) => f(s.wl_surface()),
                smithay::desktop::WindowSurface::X11(s) => f(s),
            }
            Self::LayerSurface(l) => f(l.wl_surface()),
            Self::Popup(p) => f(p.wl_surface()),
        }
    }
)]
impl smithay::input::keyboard::KeyboardTarget<SabiniwmState> for KeyboardFocusTarget {}

impl smithay::wayland::seat::WaylandFocus for KeyboardFocusTarget {
    fn wl_surface(&self) -> Option<WlSurface> {
        match self {
            KeyboardFocusTarget::Window(w) => w.wl_surface(),
            KeyboardFocusTarget::LayerSurface(l) => Some(l.wl_surface().clone()),
            KeyboardFocusTarget::Popup(p) => Some(p.wl_surface().clone()),
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
            KeyboardFocusTarget::Window(w) => match w.underlying_surface() {
                WindowSurface::Wayland(s) => PointerFocusTarget::from(s.wl_surface().clone()),
                WindowSurface::X11(s) => PointerFocusTarget::from(s.clone()),
            },
            KeyboardFocusTarget::LayerSurface(l) => {
                PointerFocusTarget::from(l.wl_surface().clone())
            }
            KeyboardFocusTarget::Popup(p) => PointerFocusTarget::from(p.wl_surface().clone()),
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
