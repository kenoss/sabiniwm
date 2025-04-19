use crate::backend::BackendI;
use crate::state::SabiniwmState;
use crate::view::window::Window;
use crate::ClientState;
use smithay::backend::renderer::utils::on_commit_buffer_handler;
use smithay::desktop::{layer_map_for_output, LayerSurface};
use smithay::output::Output;
use smithay::reexports::calloop::Interest;
use smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer;
use smithay::reexports::wayland_server::protocol::wl_output;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{Client, Resource};
use smithay::utils::{Logical, Rectangle};
use smithay::wayland::buffer::BufferHandler;
use smithay::wayland::compositor::{
    add_blocker, add_pre_commit_hook, get_parent, is_sync_subsurface, with_states,
    BufferAssignment, CompositorClientState, CompositorHandler, CompositorState, SurfaceAttributes,
};
use smithay::wayland::dmabuf::get_dmabuf;
use smithay::wayland::seat::WaylandFocus;
use smithay::wayland::shell::wlr_layer::{
    Layer, LayerSurface as WlrLayerSurface, WlrLayerShellHandler, WlrLayerShellState,
};
use smithay::xwayland::XWaylandClientData;

mod x11;
mod xdg;

impl BufferHandler for SabiniwmState {
    fn buffer_destroyed(&mut self, _buffer: &WlBuffer) {}
}

impl CompositorHandler for SabiniwmState {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.inner.compositor_state
    }

    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        if let Some(state) = client.get_data::<XWaylandClientData>() {
            return &state.compositor_state;
        }

        if let Some(state) = client.get_data::<ClientState>() {
            return &state.compositor_state;
        }

        panic!("Unknown client data type")
    }

    fn new_surface(&mut self, surface: &WlSurface) {
        add_pre_commit_hook::<Self, _>(surface, move |state, _dh, surface| {
            let (acquire_point, maybe_dmabuf) = with_states(surface, |surface_data| {
                use smithay::wayland::drm_syncobj::DrmSyncobjCachedState;

                (
                    surface_data
                        .cached_state
                        .get::<DrmSyncobjCachedState>()
                        .pending()
                        .acquire_point
                        .clone(),
                    surface_data
                        .cached_state
                        .get::<SurfaceAttributes>()
                        .pending()
                        .buffer
                        .as_ref()
                        .and_then(|assignment| match assignment {
                            BufferAssignment::NewBuffer(buffer) => get_dmabuf(buffer).ok().cloned(),
                            _ => None,
                        }),
                )
            });
            if let Some(dmabuf) = maybe_dmabuf {
                if let Some(acquire_point) = acquire_point {
                    if let Ok((blocker, source)) = acquire_point.generate_blocker() {
                        let client = surface.client().unwrap();
                        let res =
                            state
                                .inner
                                .loop_handle
                                .insert_source(source, move |_, _, state| {
                                    let display_handle = state.inner.display_handle.clone();
                                    state
                                        .client_compositor_state(&client)
                                        .blocker_cleared(state, &display_handle);
                                    Ok(())
                                });
                        if res.is_ok() {
                            add_blocker(surface, blocker);
                            return;
                        }
                    }
                }

                if let Ok((blocker, source)) = dmabuf.generate_blocker(Interest::READ) {
                    if let Some(client) = surface.client() {
                        let res =
                            state
                                .inner
                                .loop_handle
                                .insert_source(source, move |_, _, state| {
                                    state.client_compositor_state(&client).blocker_cleared(
                                        state,
                                        &state.inner.display_handle.clone(),
                                    );
                                    Ok(())
                                });
                        if res.is_ok() {
                            add_blocker(surface, blocker);
                        }
                    }
                }
            }
        });
    }

    fn commit(&mut self, surface: &WlSurface) {
        on_commit_buffer_handler::<Self>(surface);

        // Process mapping and run manage hook
        //
        // Typical sequence of Wayland/X in sabiniwm is the following:
        //
        // - ...
        // - (client) xdg_surface::get_toplevel
        // - ...
        // - (client) wl_surface::commit
        // - (server) xdg_surface::configure
        // - ...
        // - (client) wl_surface::attach
        // - ...
        // - (client) wl_surface::commit
        // - (server) Process mapping and run manage hook
        //
        // - ...
        // - (client) PropertyNotify
        // - ...
        // - (client) MapRequest
        // - (server) Process mapping and run manage hook
        //
        // We run manage hook at this timing because manage hook needs properties like app id,
        // title, modal or not, buffer size, etc.
        let mut process_initial_mapping = || {
            use smithay::reexports::wayland_server::Resource;

            // Do nothing if not in initial mapping.
            if !self
                .inner
                .windows_waiting_mapping
                .contains_key(&surface.id())
            {
                return;
            }

            let has_buffer = smithay::backend::renderer::utils::with_renderer_surface_state(surface, |state| {
                    state.buffer().is_some()
                }).unwrap(/* on_commit_buffer_handler() is called */);
            if !has_buffer {
                // If a buffer is not attached yet (e.g. initial `wl_surface::commit`), send
                // `xdg_surface::configure` to let the client send `wl_surface::attach`.
                // It is for getting what buffer size the client wants to show, which is used
                // appropriate floating in a manage hook.

                let window = self
                    .inner
                    .windows_waiting_mapping
                    .get(&surface.id())
                    .unwrap();
                let surface =
                    window.toplevel().unwrap(/* window is added by xdg_surface::get_toplevel */);
                surface.with_pending_state(|state| {
                    use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;

                    state.states.set(xdg_toplevel::State::Fullscreen);
                    state.states.set(xdg_toplevel::State::TiledTop);
                    state.states.set(xdg_toplevel::State::TiledLeft);
                    state.states.set(xdg_toplevel::State::TiledBottom);
                    state.states.set(xdg_toplevel::State::TiledRight);
                });
                surface.send_pending_configure();
            } else {
                // Otherwise, we process mapping and run manage hook.

                let window = self
                    .inner
                    .windows_waiting_mapping
                    .remove(&surface.id())
                    .unwrap();
                let window_id = self.inner.view.register_window(window);
                self.inner.view.run_manage_hook(
                    &self.inner.config_delegate,
                    window_id,
                    self.inner.display_handle.clone(),
                );
                self.inner.view.layout(&mut self.inner.space);
                self.reflect_focus_from_stackset();
            }
        };
        process_initial_mapping();

        self.backend.early_import(surface);

        if !is_sync_subsurface(surface) {
            let mut root = surface.clone();
            while let Some(parent) = get_parent(&root) {
                root = parent;
            }
            if let Some(window) = self.window_for_surface(&root) {
                window.smithay_window().on_commit();
            }
        }
        self.inner.popups.commit(surface);

        use smithay::input::pointer::{CursorImageStatus, CursorImageSurfaceData};
        if matches!(&self.inner.cursor_status, CursorImageStatus::Surface(cursor_surface) if cursor_surface == surface)
        {
            with_states(surface, |states| {
                let cursor_image_attributes = states.data_map.get::<CursorImageSurfaceData>();

                if let Some(mut cursor_image_attributes) =
                    cursor_image_attributes.map(|attrs| attrs.lock().unwrap())
                {
                    let buffer_delta = states
                        .cached_state
                        .get::<SurfaceAttributes>()
                        .current()
                        .buffer_delta
                        .take();
                    if let Some(buffer_delta) = buffer_delta {
                        trace!(hotspot = ?cursor_image_attributes.hotspot, ?buffer_delta, "decrementing cursor hotspot");
                        cursor_image_attributes.hotspot -= buffer_delta;
                    }
                }
            });
        }

        if matches!(&self.inner.dnd_icon, Some(icon) if &icon.surface == surface) {
            let dnd_icon = self.inner.dnd_icon.as_mut().unwrap();
            with_states(&dnd_icon.surface, |states| {
                let buffer_delta = states
                    .cached_state
                    .get::<SurfaceAttributes>()
                    .current()
                    .buffer_delta
                    .take()
                    .unwrap_or_default();
                trace!(offset = ?dnd_icon.offset, ?buffer_delta, "moving dnd offset");
                dnd_icon.offset += buffer_delta;
            });
        }

        ensure_initial_configure(surface, &self.inner.space, &mut self.inner.popups)
    }
}

impl WlrLayerShellHandler for SabiniwmState {
    fn shell_state(&mut self) -> &mut WlrLayerShellState {
        &mut self.inner.layer_shell_state
    }

    fn new_layer_surface(
        &mut self,
        surface: WlrLayerSurface,
        wl_output: Option<wl_output::WlOutput>,
        _layer: Layer,
        namespace: String,
    ) {
        let output = wl_output
            .as_ref()
            .and_then(Output::from_resource)
            .unwrap_or_else(|| self.inner.space.outputs().next().unwrap().clone());
        let mut map = layer_map_for_output(&output);
        map.map_layer(&LayerSurface::new(surface, namespace))
            .unwrap();
    }

    fn layer_destroyed(&mut self, surface: WlrLayerSurface) {
        if let Some((mut map, layer)) = self.inner.space.outputs().find_map(|o| {
            let map = layer_map_for_output(o);
            let layer = map
                .layers()
                .find(|&layer| layer.layer_surface() == &surface)
                .cloned();
            layer.map(|layer| (map, layer))
        }) {
            map.unmap_layer(&layer);
        }
    }
}

impl SabiniwmState {
    pub fn window_for_surface(&self, surface: &WlSurface) -> Option<crate::view::window::Window> {
        self.inner
            .space
            .elements()
            .find(|window| window.smithay_window().wl_surface().as_deref() == Some(surface))
            .cloned()
    }
}

#[derive(Default)]
pub struct SurfaceData {
    pub geometry: Option<Rectangle<i32, Logical>>,
}

fn ensure_initial_configure(
    surface: &WlSurface,
    space: &smithay::desktop::Space<Window>,
    popups: &mut smithay::desktop::PopupManager,
) {
    use smithay::desktop::{PopupKind, WindowSurfaceType};
    use smithay::wayland::compositor::{with_surface_tree_upward, TraversalAction};
    use smithay::wayland::shell::wlr_layer::LayerSurfaceData;
    use smithay::wayland::shell::xdg::{XdgPopupSurfaceData, XdgToplevelSurfaceData};
    use std::cell::RefCell;

    with_surface_tree_upward(
        surface,
        (),
        |_, _, _| TraversalAction::DoChildren(()),
        |_, states, _| {
            states
                .data_map
                .insert_if_missing(|| RefCell::new(SurfaceData::default()));
        },
        |_, _, _| true,
    );

    if let Some(window) = space
        .elements()
        .find(|window| window.smithay_window().wl_surface().as_deref() == Some(surface))
        .cloned()
    {
        // send the initial configure if relevant
        #[cfg_attr(not(feature = "xwayland"), allow(irrefutable_let_patterns))]
        if let Some(toplevel) = window.smithay_window().toplevel() {
            let initial_configure_sent = with_states(surface, |states| {
                states
                    .data_map
                    .get::<XdgToplevelSurfaceData>()
                    .unwrap()
                    .lock()
                    .unwrap()
                    .initial_configure_sent
            });
            if !initial_configure_sent {
                toplevel.send_configure();
            }
        }

        return;
    }

    if let Some(popup) = popups.find_popup(surface) {
        let popup = match &popup {
            PopupKind::Xdg(popup) => popup,
            // Doesn't require configure
            PopupKind::InputMethod(_) => {
                return;
            }
        };

        let initial_configure_sent = with_states(surface, |states| {
            states
                .data_map
                .get::<XdgPopupSurfaceData>()
                .unwrap()
                .lock()
                .unwrap()
                .initial_configure_sent
        });
        if !initial_configure_sent {
            // NOTE: This should never fail as the initial configure is always
            // allowed.
            popup.send_configure().expect("initial configure failed");
        }

        return;
    };

    if let Some(output) = space.outputs().find(|o| {
        let map = layer_map_for_output(o);
        map.layer_for_surface(surface, WindowSurfaceType::TOPLEVEL)
            .is_some()
    }) {
        let initial_configure_sent = with_states(surface, |states| {
            states
                .data_map
                .get::<LayerSurfaceData>()
                .unwrap()
                .lock()
                .unwrap()
                .initial_configure_sent
        });

        let mut map = layer_map_for_output(output);

        // arrange the layers before sending the initial configure
        // to respect any size the client may have sent
        map.arrange();
        // send the initial configure if relevant
        if !initial_configure_sent {
            let layer = map
                .layer_for_surface(surface, WindowSurfaceType::TOPLEVEL)
                .unwrap();

            layer.layer_surface().send_configure();
        }
    };
}
