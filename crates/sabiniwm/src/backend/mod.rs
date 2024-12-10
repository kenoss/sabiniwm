pub mod udev;
#[cfg(feature = "winit")]
pub mod winit;

#[thin_delegate::register]
pub(crate) trait DmabufHandlerDelegate: smithay::wayland::buffer::BufferHandler {
    fn dmabuf_state(&mut self) -> &mut smithay::wayland::dmabuf::DmabufState;
    fn dmabuf_imported(
        &mut self,
        global: &smithay::wayland::dmabuf::DmabufGlobal,
        dmabuf: smithay::backend::allocator::dmabuf::Dmabuf,
    ) -> bool;
}

#[thin_delegate::register]
pub(crate) trait BackendI: DmabufHandlerDelegate {
    fn init(&mut self, inner: &mut crate::state::InnerState) -> eyre::Result<()>;
    fn has_relative_motion(&self) -> bool;
    fn has_gesture(&self) -> bool;
    fn seat_name(&self) -> String;
    fn early_import(&mut self, surface: &wayland_server::protocol::wl_surface::WlSurface);
    fn update_led_state(&mut self, led_state: smithay::input::keyboard::LedState);
    fn change_vt(&mut self, vt: i32);
}

#[derive(derive_more::From)]
#[thin_delegate::register]
pub(crate) enum Backend {
    Udev(udev::UdevBackend),
    #[cfg(feature = "winit")]
    Winit(winit::WinitBackend),
}

#[thin_delegate::derive_delegate(
    external_trait_def = crate::external_trait_def::smithay::wayland::buffer,
    scheme = |f| {
        match self {
            Self::Udev(backend) => f(backend),
            #[cfg(feature = "winit")]
            Self::Winit(backend) => f(backend),
        }
    }
)]
impl smithay::wayland::buffer::BufferHandler for Backend {}

#[thin_delegate::derive_delegate(
    scheme = |f| {
        match self {
            Self::Udev(backend) => f(backend),
            #[cfg(feature = "winit")]
            Self::Winit(backend) => f(backend),
        }
    }
)]
impl DmabufHandlerDelegate for Backend {}

#[thin_delegate::derive_delegate(
    scheme = |f| {
        match self {
            Self::Udev(backend) => f(backend),
            #[cfg(feature = "winit")]
            Self::Winit(backend) => f(backend),
        }
    }
)]
impl BackendI for Backend {}

impl Backend {
    fn as_udev(&self) -> &udev::UdevBackend {
        match self {
            Self::Udev(backend) => backend,
            #[cfg(feature = "winit")]
            Self::Winit(_) => unreachable!(),
        }
    }

    fn as_udev_mut(&mut self) -> &mut udev::UdevBackend {
        match self {
            Self::Udev(backend) => backend,
            #[cfg(feature = "winit")]
            Self::Winit(_) => unreachable!(),
        }
    }

    #[cfg(feature = "winit")]
    fn as_winit_mut(&mut self) -> &mut winit::WinitBackend {
        match self {
            Self::Udev(_) => unreachable!(),
            Self::Winit(backend) => backend,
        }
    }

    pub(crate) fn is_udev(&self) -> bool {
        match self {
            Self::Udev(_) => true,
            #[cfg(feature = "winit")]
            Self::Winit(_) => false,
        }
    }
}
