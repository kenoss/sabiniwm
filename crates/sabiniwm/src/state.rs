use crate::action::Action;
use crate::backend::{Backend, BackendI};
use crate::config::{ConfigDelegate, ConfigDelegateUnstableI};
use crate::cursor::Cursor;
use crate::envvar::EnvVar;
use crate::input::{KeySeq, Keymap};
use crate::input_event::FocusUpdateDecider;
use crate::util::EventHandler;
use crate::view::view::View;
use crate::view::window::Window;
use eyre::WrapErr;
use smithay::desktop::{PopupManager, Space};
use smithay::input::pointer::{CursorImageStatus, PointerHandle};
use smithay::input::{Seat, SeatState};
use smithay::reexports::calloop::{EventLoop, LoopHandle, LoopSignal};
use smithay::reexports::wayland_server;
use smithay::reexports::wayland_server::backend::{ClientData, ClientId, DisconnectReason};
use smithay::reexports::wayland_server::{Display, DisplayHandle};
use smithay::utils::{Clock, Monotonic, Point, Rectangle, Size};
use smithay::wayland::compositor::{CompositorClientState, CompositorState};
use smithay::wayland::input_method::InputMethodManagerState;
use smithay::wayland::keyboard_shortcuts_inhibit::KeyboardShortcutsInhibitState;
use smithay::wayland::pointer_constraints::PointerConstraintsState;
use smithay::wayland::pointer_gestures::PointerGesturesState;
use smithay::wayland::relative_pointer::RelativePointerManagerState;
use smithay::wayland::security_context::{SecurityContext, SecurityContextState};
use smithay::wayland::selection::data_device::DataDeviceState;
use smithay::wayland::selection::primary_selection::PrimarySelectionState;
use smithay::wayland::selection::wlr_data_control::DataControlState;
use smithay::wayland::shell::wlr_layer::WlrLayerShellState;
use smithay::wayland::shell::xdg::XdgShellState;
use smithay::wayland::shm::ShmState;
use smithay::wayland::single_pixel_buffer::SinglePixelBufferState;
use smithay::wayland::socket::ListeningSocketSource;
use smithay::wayland::tablet_manager::TabletManagerState;
use smithay::wayland::text_input::TextInputManagerState;
use smithay::wayland::virtual_keyboard::VirtualKeyboardManagerState;
use smithay::wayland::xdg_activation::XdgActivationState;
use smithay::wayland::xdg_foreign::XdgForeignState;
use smithay::wayland::xwayland_keyboard_grab::XWaylandKeyboardGrabState;
use smithay::xwayland::{X11Wm, XWayland, XWaylandEvent};
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Default)]
pub struct ClientState {
    pub compositor_state: CompositorClientState,
    pub security_context: Option<SecurityContext>,
}

impl ClientData for ClientState {
    /// Notification that a client was initialized
    fn initialized(&self, _client_id: ClientId) {}
    /// Notification that a client is disconnected
    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {}
}

pub struct SabiniwmState {
    pub(crate) backend: Backend,
    pub(crate) inner: InnerState,
}

pub(crate) struct InnerState {
    pub display_handle: DisplayHandle,
    pub loop_handle: LoopHandle<'static, SabiniwmState>,
    pub loop_signal: LoopSignal,

    // desktop
    pub space: Space<Window>,
    pub popups: PopupManager,

    // smithay state
    pub compositor_state: CompositorState,
    pub data_device_state: DataDeviceState,
    pub layer_shell_state: WlrLayerShellState,
    pub primary_selection_state: PrimarySelectionState,
    pub data_control_state: DataControlState,
    pub seat_state: SeatState<SabiniwmState>,
    pub keyboard_shortcuts_inhibit_state: KeyboardShortcutsInhibitState,
    pub shm_state: ShmState,
    pub xdg_activation_state: XdgActivationState,
    pub xdg_shell_state: XdgShellState,
    pub xdg_foreign_state: XdgForeignState,
    #[allow(unused)]
    pub single_pixel_buffer_state: SinglePixelBufferState,
    pub session_lock_data: crate::session_lock::SessionLockData,

    pub dnd_icon: Option<wayland_server::protocol::wl_surface::WlSurface>,

    // input-related fields
    pub cursor_status: CursorImageStatus,
    pub seat_name: String,
    pub seat: Seat<SabiniwmState>,
    pub clock: Clock<Monotonic>,
    pub pointer: PointerHandle<SabiniwmState>,

    pub xwayland_client: wayland_server::Client,
    pub xwm: Option<X11Wm>,
    pub xdisplay: Option<u32>,
    pub xwayland_shell_state: smithay::wayland::xwayland_shell::XWaylandShellState,

    pub envvar: EnvVar,
    pub keymap: Keymap<Action>,
    pub keyseq: KeySeq,
    pub view: View,
    pub focus_update_decider: FocusUpdateDecider,

    pub config_delegate: ConfigDelegate,
}

pub(crate) struct SabiniwmStateWithConcreteBackend<'a, B>
where
    B: BackendI,
{
    pub backend: &'a mut B,
    pub inner: &'a mut InnerState,
}

impl SabiniwmState {
    pub fn run(config_delegate: Box<dyn ConfigDelegateUnstableI>) -> eyre::Result<()> {
        use crate::backend::udev::UdevBackend;
        #[cfg(feature = "winit")]
        use crate::backend::winit::WinitBackend;

        let envvar = EnvVar::load()?;

        let config_delegate = ConfigDelegate::new(config_delegate);

        let event_loop = EventLoop::try_new().unwrap();

        let use_udev = envvar.generic.display.is_none() && envvar.generic.wayland_display.is_none();

        let backend = if use_udev {
            UdevBackend::new(&envvar, event_loop.handle().clone())?.into()
        } else {
            #[cfg(feature = "winit")]
            {
                WinitBackend::new(event_loop.handle().clone())?.into()
            }
            #[cfg(not(feature = "winit"))]
            {
                unreachable!();
            }
        };

        let mut this = Self::new(
            config_delegate,
            envvar,
            event_loop.handle(),
            event_loop.get_signal(),
            backend,
        )?;

        this.backend.init(&mut this.inner)?;

        this.run_loop(event_loop)?;

        Ok(())
    }

    fn new(
        config_delegate: ConfigDelegate,
        envvar: EnvVar,
        loop_handle: LoopHandle<'static, SabiniwmState>,
        loop_signal: LoopSignal,
        backend: Backend,
    ) -> eyre::Result<SabiniwmState> {
        crate::util::panic::set_hook();

        let display = Display::new().unwrap();
        let display_handle = display.handle();

        {
            use smithay::reexports::calloop::generic::Generic;
            use smithay::reexports::calloop::{Interest, Mode, PostAction};

            loop_handle
                .insert_source(
                    Generic::new(display, Interest::READ, Mode::Level),
                    |_, display, state| {
                        // Safety: we don't drop the display
                        unsafe {
                            display.get_mut().dispatch_clients(state).unwrap();
                        }
                        Ok(PostAction::Continue)
                    },
                )
                .map_err(|e| eyre::eyre!("{}", e))?;
        }

        // Initialize `WAYLAND_DISPLAY` socket to listen Wayland clients.
        let socket_source = ListeningSocketSource::new_auto()?;
        let socket_name = socket_source.socket_name().to_string_lossy().into_owned();
        loop_handle
            .insert_source(socket_source, |client_stream, _, state| {
                if let Err(err) = state
                    .inner
                    .display_handle
                    .insert_client(client_stream, Arc::new(ClientState::default()))
                {
                    warn!("Error adding wayland client: {}", err);
                };
            })
            .map_err(|e| eyre::eyre!("{}", e))?;
        std::env::set_var("WAYLAND_DISPLAY", &socket_name);
        info!(
            "Start listening on Wayland socket: WAYLAND_DISPLAY = {}",
            socket_name
        );

        // init globals
        let compositor_state = CompositorState::new::<Self>(&display_handle);
        let data_device_state = DataDeviceState::new::<Self>(&display_handle);
        let layer_shell_state = WlrLayerShellState::new::<Self>(&display_handle);
        let primary_selection_state = PrimarySelectionState::new::<Self>(&display_handle);
        let data_control_state = DataControlState::new::<Self, _>(
            &display_handle,
            Some(&primary_selection_state),
            |_| true,
        );
        let mut seat_state = SeatState::new();
        let shm_state = ShmState::new::<Self>(&display_handle, vec![]);
        let xdg_activation_state = XdgActivationState::new::<Self>(&display_handle);
        let xdg_shell_state = XdgShellState::new::<Self>(&display_handle);
        let xdg_foreign_state = XdgForeignState::new::<Self>(&display_handle);
        let single_pixel_buffer_state = SinglePixelBufferState::new::<Self>(&display_handle);
        let session_lock_data = crate::session_lock::SessionLockData::new(&display_handle);
        TextInputManagerState::new::<Self>(&display_handle);
        InputMethodManagerState::new::<Self, _>(&display_handle, |_client| true);
        VirtualKeyboardManagerState::new::<Self, _>(&display_handle, |_client| true);
        if backend.has_relative_motion() {
            RelativePointerManagerState::new::<Self>(&display_handle);
        }
        PointerConstraintsState::new::<Self>(&display_handle);
        if backend.has_gesture() {
            PointerGesturesState::new::<Self>(&display_handle);
        }
        TabletManagerState::new::<Self>(&display_handle);
        SecurityContextState::new::<Self, _>(&display_handle, |client| {
            client
                .get_data::<ClientState>()
                .map_or(true, |client_state| client_state.security_context.is_none())
        });

        // init input
        let seat_name = backend.seat_name();
        let mut seat = seat_state.new_wl_seat(&display_handle, seat_name.clone());

        let cursor_status = CursorImageStatus::default_named();
        let pointer = seat.add_pointer();

        let xkb_config = config_delegate.get_xkb_config();
        seat.add_keyboard(
            xkb_config.xkb_config,
            xkb_config.repeat_delay.into(),
            xkb_config.repeat_rate.into(),
        )
        .unwrap();

        let keyboard_shortcuts_inhibit_state =
            KeyboardShortcutsInhibitState::new::<Self>(&display_handle);

        let xwayland_client = {
            use std::process::Stdio;

            XWaylandKeyboardGrabState::new::<Self>(&display_handle.clone());

            let (xwayland, xwayland_client) = XWayland::spawn(
                &display_handle,
                None,
                std::iter::empty::<(String, String)>(),
                true,
                Stdio::null(),
                Stdio::null(),
                |_| (),
            )
            .wrap_err("XWayland::spawn()")?;

            loop_handle
                .insert_source(xwayland, move |event, _, state| state.handle_event(event))
                .map_err(|e| eyre::eyre!("{}", e))?;

            xwayland_client
        };
        let xwayland_shell_state = smithay::wayland::xwayland_shell::XWaylandShellState::new::<Self>(
            &display_handle.clone(),
        );

        let keymap = config_delegate.make_keymap(backend.is_udev());

        let rect = Rectangle::from_loc_and_size((0, 0), (1280, 720));
        let view = View::new(&config_delegate, rect);

        Ok(SabiniwmState {
            backend,
            inner: InnerState {
                display_handle,
                loop_handle,
                loop_signal,
                space: Space::default(),
                popups: PopupManager::default(),
                compositor_state,
                data_device_state,
                layer_shell_state,
                primary_selection_state,
                data_control_state,
                seat_state,
                keyboard_shortcuts_inhibit_state,
                shm_state,
                xdg_activation_state,
                xdg_shell_state,
                xdg_foreign_state,
                single_pixel_buffer_state,
                session_lock_data,
                dnd_icon: None,
                cursor_status,
                seat_name,
                seat,
                pointer,
                clock: Clock::new(),
                xwayland_client,
                xwm: None,
                xdisplay: None,
                xwayland_shell_state,

                envvar,
                keymap,
                keyseq: KeySeq::new(),
                view,
                focus_update_decider: FocusUpdateDecider::new(),

                config_delegate,
            },
        })
    }

    fn run_loop(&mut self, mut event_loop: EventLoop<'_, SabiniwmState>) -> eyre::Result<()> {
        event_loop.run(None, self, |state| {
            let should_reflect = state.inner.view.refresh(&mut state.inner.space);
            if should_reflect {
                state.reflect_focus_from_stackset();
            }

            state.inner.space.refresh();
            state.inner.popups.cleanup();
            state.inner.display_handle.flush_clients().unwrap();
        })?;

        Ok(())
    }
}

impl InnerState {
    pub fn on_output_added(&mut self, output: &smithay::output::Output) {
        self.session_lock_data.on_output_added(output);
    }
}

impl EventHandler<XWaylandEvent> for SabiniwmState {
    fn handle_event(&mut self, event: XWaylandEvent) {
        match event {
            XWaylandEvent::Ready {
                x11_socket,
                display_number,
            } => {
                let mut wm = X11Wm::start_wm(
                    self.inner.loop_handle.clone(),
                    x11_socket,
                    self.inner.xwayland_client.clone(),
                )
                .expect("attach X11 Window Manager");

                let cursor = Cursor::load();
                let image = cursor.get_image(1, Duration::ZERO);
                wm.set_cursor(
                    &image.pixels_rgba,
                    Size::from((image.width as u16, image.height as u16)),
                    Point::from((image.xhot as u16, image.yhot as u16)),
                )
                .expect("set xwayland default cursor");

                self.inner.xwm = Some(wm);
                self.inner.xdisplay = Some(display_number);
            }
            XWaylandEvent::Error => {
                warn!("XWayland crashed on startup");
            }
        }
    }
}
