use crate::backend::BackendI;
use crate::state::SabiniwmState;
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
use smithay::xwayland::{X11Wm, XWaylandClientData};

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
            let maybe_dmabuf = with_states(surface, |surface_data| {
                surface_data
                    .cached_state
                    .pending::<SurfaceAttributes>()
                    .buffer
                    .as_ref()
                    .and_then(|assignment| match assignment {
                        BufferAssignment::NewBuffer(buffer) => get_dmabuf(buffer).ok(),
                        _ => None,
                    })
            });
            if let Some(dmabuf) = maybe_dmabuf {
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
        X11Wm::commit_hook::<SabiniwmState>(surface);

        on_commit_buffer_handler::<Self>(surface);
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
            .find(|window| {
                window
                    .smithay_window()
                    .wl_surface()
                    .map(|s| s == *surface)
                    .unwrap_or(false)
            })
            .cloned()
    }
}

#[derive(Default)]
pub struct SurfaceData {
    pub geometry: Option<Rectangle<i32, Logical>>,
}
