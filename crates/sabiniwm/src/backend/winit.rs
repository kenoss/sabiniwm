use crate::backend::BackendI;
use crate::pointer::PointerElement;
use crate::render::CustomRenderElement;
use crate::render_loop::RenderLoop;
use crate::state::{InnerState, SabiniwmState, SabiniwmStateWithConcreteBackend};
use crate::util::EventHandler;
use eyre::WrapErr;
use smithay::backend::egl::EGLDevice;
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::element::AsRenderElements;
use smithay::backend::renderer::gles::GlesRenderer;
#[cfg(feature = "egl")]
use smithay::backend::renderer::ImportEgl;
use smithay::backend::renderer::{ImportDma, ImportMemWl};
use smithay::backend::winit::{self, WinitEvent, WinitGraphicsBackend};
use smithay::backend::SwapBuffersError;
use smithay::input::pointer::{CursorImageAttributes, CursorImageStatus};
use smithay::output::{Mode, PhysicalProperties, Subpixel};
use smithay::reexports::calloop::LoopHandle;
use smithay::reexports::wayland_protocols::wp::presentation_time::server::wp_presentation_feedback;
use smithay::reexports::wayland_server;
use smithay::reexports::wayland_server::protocol::wl_surface;
use smithay::utils::{IsAlive, Scale, Transform};
use smithay::wayland::compositor;
use smithay::wayland::dmabuf::{DmabufFeedback, DmabufFeedbackBuilder, DmabufGlobal, DmabufState};
use std::cell::OnceCell;
use std::sync::Mutex;
use std::time::Duration;

const OUTPUT_NAME: &str = "winit";

pub(crate) struct WinitBackend {
    backend: WinitGraphicsBackend<GlesRenderer>,
    output: smithay::output::Output,
    render_loop: RenderLoop<SabiniwmState>,
    damage_tracker: OutputDamageTracker,
    dmabuf_state: DmabufState,
    dmabuf_global: OnceCell<DmabufGlobal>,
    dmabuf_feedback: Option<DmabufFeedback>,
    full_redraw: u8,
    pointer_element: PointerElement,
}

impl WinitBackend {
    pub(crate) fn new(loop_handle: LoopHandle<'static, SabiniwmState>) -> eyre::Result<Self> {
        let (backend, winit_event_loop) = winit::init::<GlesRenderer>()
            .map_err(|e| eyre::eyre!("{}", e))
            .wrap_err("initializing winit backend")?;

        loop_handle
            .insert_source(winit_event_loop, move |event, _, state| {
                state.handle_event(event)
            })
            .map_err(|e| eyre::eyre!("{}", e))?;

        let output = smithay::output::Output::new(
            OUTPUT_NAME.to_string(),
            PhysicalProperties {
                size: (0, 0).into(),
                subpixel: Subpixel::Unknown,
                make: "Smithay".into(),
                model: "Winit".into(),
            },
        );
        let mode = Mode {
            size: backend.window_size(),
            refresh: 60_000,
        };
        output.change_current_state(
            Some(mode),
            Some(Transform::Flipped180),
            None,
            Some((0, 0).into()),
        );
        output.set_preferred(mode);
        // `InnerState::on_output_added()` will be called later, at the head of `init()`, as it requires `InnerState`.

        let mut render_loop = RenderLoop::new(loop_handle.clone(), &output, move |state| {
            state.as_winit_mut().render();
        });
        render_loop.start();

        let damage_tracker = OutputDamageTracker::from_output(&output);

        let pointer_element = PointerElement::default();

        Ok(WinitBackend {
            backend,
            output,
            render_loop,
            damage_tracker,
            dmabuf_state: DmabufState::new(),
            dmabuf_global: OnceCell::new(),
            dmabuf_feedback: None,
            full_redraw: 0,
            pointer_element,
        })
    }
}

impl smithay::wayland::buffer::BufferHandler for WinitBackend {
    fn buffer_destroyed(&mut self, _buffer: &wayland_server::protocol::wl_buffer::WlBuffer) {}
}

impl crate::backend::DmabufHandlerDelegate for WinitBackend {
    fn dmabuf_state(&mut self) -> &mut smithay::wayland::dmabuf::DmabufState {
        &mut self.dmabuf_state
    }

    fn dmabuf_imported(
        &mut self,
        _global: &smithay::wayland::dmabuf::DmabufGlobal,
        dmabuf: smithay::backend::allocator::dmabuf::Dmabuf,
    ) -> bool {
        self.backend.renderer().import_dmabuf(&dmabuf, None).is_ok()
    }
}

impl BackendI for WinitBackend {
    fn init(&mut self, inner: &mut InnerState) -> eyre::Result<()> {
        inner.on_output_added(&self.output);

        #[cfg(feature = "egl")]
        if self
            .backend
            .renderer()
            .bind_wl_display(&inner.display_handle)
            .is_ok()
        {
            info!("EGL hardware-acceleration enabled");
        };

        let render_node =
            EGLDevice::device_for_display(self.backend.renderer().egl_context().display())
                .and_then(|device| device.try_get_render_node());
        self.dmabuf_feedback = match render_node {
            Ok(Some(node)) => {
                let dmabuf_default_feedback = DmabufFeedbackBuilder::new(
                    node.dev_id(),
                    self.backend.renderer().dmabuf_formats(),
                )
                .build()?;
                Some(dmabuf_default_feedback)
            }
            Ok(None) => {
                warn!("failed to query render node, dmabuf will use v3");
                None
            }
            Err(err) => {
                warn!(?err, "failed to egl device for display, dmabuf will use v3");
                None
            }
        };
        let dmabuf_global = if let Some(dmabuf_feedback) = &self.dmabuf_feedback {
            self.dmabuf_state
                .create_global_with_default_feedback::<SabiniwmState>(
                    &inner.display_handle,
                    dmabuf_feedback,
                )
        } else {
            // If we failed to build dmabuf feedback, we fall back to dmabuf v3.
            // Note: egl on Mesa requires either v4 or wl_drm (initialized with bind_wl_display).
            self.dmabuf_state.create_global::<SabiniwmState>(
                &inner.display_handle,
                self.backend.renderer().dmabuf_formats().collect(),
            )
        };
        self.dmabuf_global.set(dmabuf_global).unwrap();

        inner
            .shm_state
            .update_formats(self.backend.renderer().shm_formats());

        inner.space.map_output(&self.output, (0, 0));

        Ok(())
    }

    fn has_relative_motion(&self) -> bool {
        false
    }

    fn has_gesture(&self) -> bool {
        false
    }

    fn seat_name(&self) -> String {
        String::from("winit")
    }

    fn early_import(&mut self, _surface: &wl_surface::WlSurface) {}

    fn update_led_state(&mut self, _led_state: smithay::input::keyboard::LedState) {}

    fn change_vt(&mut self, _vt: i32) {
        error!("changing VT is not supported on winit backend");
    }
}

impl EventHandler<WinitEvent> for SabiniwmState {
    fn handle_event(&mut self, event: WinitEvent) {
        match event {
            WinitEvent::CloseRequested => {
                self.inner.loop_signal.stop();
            }
            WinitEvent::Input(event) => {
                use smithay::backend::input::InputEvent;

                match event {
                    InputEvent::DeviceAdded { .. } | InputEvent::DeviceRemoved { .. } => {}
                    _ => {
                        self.process_input_event(event);
                    }
                }
            }
            WinitEvent::Resized { size, .. } => {
                let this = self.as_winit_mut();
                let output = &mut this.backend.output;
                let mode = Mode {
                    size,
                    refresh: 60_000,
                };
                output.set_preferred(mode);
                output.change_current_state(Some(mode), None, None, None);
                this.inner.space.map_output(output, (0, 0));
                let size = this.inner.space.output_geometry(output)
                    .unwrap(/* Space::map_output() and Output::change_current_state() is called. */)
                    .size;
                this.inner.view.resize_output(size, &mut this.inner.space);
            }
            WinitEvent::Focus(_) | WinitEvent::Redraw => {}
        }
    }
}

impl SabiniwmState {
    fn as_winit_mut(&mut self) -> SabiniwmStateWithConcreteBackend<'_, WinitBackend> {
        SabiniwmStateWithConcreteBackend {
            backend: self.backend.as_winit_mut(),
            inner: &mut self.inner,
        }
    }
}

impl SabiniwmStateWithConcreteBackend<'_, WinitBackend> {
    fn render(&mut self) {
        // draw the cursor as relevant
        // reset the cursor if the surface is no longer alive
        let mut reset = false;
        if let CursorImageStatus::Surface(ref surface) = self.inner.cursor_status {
            reset = !surface.alive();
        }
        if reset {
            self.inner.cursor_status = CursorImageStatus::default_named();
        }
        let cursor_visible = !matches!(self.inner.cursor_status, CursorImageStatus::Surface(_));

        self.backend
            .pointer_element
            .set_status(self.inner.cursor_status.clone());

        let full_redraw = &mut self.backend.full_redraw;
        *full_redraw = full_redraw.saturating_sub(1);
        let damage_tracker = &mut self.backend.damage_tracker;

        let dnd_icon = self.inner.dnd_icon.as_ref();

        let scale = Scale::from(self.backend.output.current_scale().fractional_scale());
        let cursor_hotspot =
            if let CursorImageStatus::Surface(ref surface) = self.inner.cursor_status {
                compositor::with_states(surface, |states| {
                    states
                        .data_map
                        .get::<Mutex<CursorImageAttributes>>()
                        .unwrap()
                        .lock()
                        .unwrap()
                        .hotspot
                })
            } else {
                (0, 0).into()
            };
        let cursor_pos = self.inner.pointer.current_location() - cursor_hotspot.to_f64();
        let cursor_pos_scaled = cursor_pos.to_physical(scale).to_i32_round();

        let render_res = self.backend.backend.bind().and_then(|_| {
            let age = if *full_redraw > 0 {
                0
            } else {
                self.backend.backend.buffer_age().unwrap_or(0)
            };

            let renderer = self.backend.backend.renderer();

            let mut elements = Vec::<CustomRenderElement<GlesRenderer>>::new();

            elements.extend(self.backend.pointer_element.render_elements(
                renderer,
                cursor_pos_scaled,
                scale,
                1.0,
            ));

            // draw the dnd icon if any
            if let Some(surface) = dnd_icon {
                if surface.alive() {
                    elements.extend(AsRenderElements::<GlesRenderer>::render_elements(
                        &smithay::desktop::space::SurfaceTree::from_surface(surface),
                        renderer,
                        cursor_pos_scaled,
                        scale,
                        1.0,
                    ));
                }
            }

            let (elements, clear_color) =
                self.inner
                    .make_output_elements(renderer, &self.backend.output, elements);
            // TODO: Integrate it with the below `match`.
            match damage_tracker.render_output(renderer, age, &elements, clear_color) {
                Ok(x) => Ok(x),
                Err(smithay::backend::renderer::damage::Error::Rendering(e)) => Err(e.into()),
                Err(_) => unreachable!(),
            }
        });

        match render_res {
            Ok(render_output_result) => {
                let has_rendered = render_output_result.damage.is_some();
                if let Some(damage) = render_output_result.damage {
                    if let Err(err) = self.backend.backend.submit(Some(damage)) {
                        warn!("Failed to submit buffer: {}", err);
                    }
                }

                self.backend
                    .backend
                    .window()
                    .set_cursor_visible(cursor_visible);

                // Send frame events so that client start drawing their next frame
                let time = self.inner.clock.now();
                self.inner.post_repaint(
                    &self.backend.output,
                    &render_output_result.states,
                    None,
                    time.into(),
                );

                if has_rendered {
                    let mut output_presentation_feedback = self.inner.take_presentation_feedback(
                        &self.backend.output,
                        &render_output_result.states,
                    );
                    output_presentation_feedback.presented(
                        time,
                        self.backend
                            .output
                            .current_mode()
                            .map(|mode| Duration::from_secs_f64(1_000f64 / mode.refresh as f64))
                            .unwrap_or_default(),
                        0,
                        wp_presentation_feedback::Kind::Vsync,
                    )
                }
            }
            Err(SwapBuffersError::ContextLost(err)) => {
                error!("Critical Rendering Error: {}", err);
                self.inner.loop_signal.stop();
            }
            Err(err) => warn!("Rendering error: {}", err),
        }

        // TODO: Use `should_schedule_render = false` and call `on_vblank()` on frame callback.
        self.backend.render_loop.on_render_frame(true);
    }
}
