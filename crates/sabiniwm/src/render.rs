use crate::pointer::{PointerRenderElement, CLEAR_COLOR};
use crate::state::InnerState;
use crate::view::window::WindowRenderElement;
use smithay::backend::renderer::element::solid::SolidColorRenderElement;
use smithay::backend::renderer::element::surface::WaylandSurfaceRenderElement;
use smithay::backend::renderer::element::{RenderElement, RenderElementStates, Wrap};
use smithay::backend::renderer::{ImportAll, ImportMem, Renderer};
use smithay::desktop::space::SpaceRenderElements;
use smithay::output::Output;
use smithay::wayland::dmabuf::DmabufFeedback;
use std::time::Duration;

#[derive(derive_more::From)]
#[thin_delegate::register]
pub enum CustomRenderElement<R>
where
    R: Renderer,
{
    Pointer(PointerRenderElement<R>),
    Surface(WaylandSurfaceRenderElement<R>),
}

#[thin_delegate::fill_delegate(external_trait_def = crate::external_trait_def::smithay::backend::renderer::element)]
impl<R> smithay::backend::renderer::element::Element for CustomRenderElement<R>
where
    R: smithay::backend::renderer::Renderer,
    <R as smithay::backend::renderer::Renderer>::TextureId: 'static,
    R: ImportAll + ImportMem,
{
}

#[thin_delegate::fill_delegate(external_trait_def = crate::external_trait_def::smithay::backend::renderer::element)]
impl<R> smithay::backend::renderer::element::RenderElement<R> for CustomRenderElement<R>
where
    R: smithay::backend::renderer::Renderer,
    <R as smithay::backend::renderer::Renderer>::TextureId: 'static,
    R: ImportAll + ImportMem,
{
}

impl<R> std::fmt::Debug for CustomRenderElement<R>
where
    R: Renderer,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pointer(arg0) => f.debug_tuple("Pointer").field(arg0).finish(),
            Self::Surface(arg0) => f.debug_tuple("Surface").field(arg0).finish(),
        }
    }
}

#[derive(derive_more::From)]
#[thin_delegate::register]
pub enum OutputRenderElement<R, E>
where
    R: Renderer,
    E: smithay::backend::renderer::element::RenderElement<R>,
{
    Space(SpaceRenderElements<R, E>),
    Window(Wrap<E>),
    Custom(CustomRenderElement<R>),
    SessionLockSurface(WaylandSurfaceRenderElement<R>),
    SessionLockBackground(SolidColorRenderElement),
}

#[thin_delegate::fill_delegate(external_trait_def = crate::external_trait_def::smithay::backend::renderer::element)]
impl<R, E> smithay::backend::renderer::element::Element for OutputRenderElement<R, E>
where
    R: smithay::backend::renderer::Renderer,
    <R as smithay::backend::renderer::Renderer>::TextureId: 'static,
    E: smithay::backend::renderer::element::Element
        + smithay::backend::renderer::element::RenderElement<R>,
    R: ImportAll + ImportMem,
{
}

#[thin_delegate::fill_delegate(external_trait_def = crate::external_trait_def::smithay::backend::renderer::element)]
impl<R, E> smithay::backend::renderer::element::RenderElement<R> for OutputRenderElement<R, E>
where
    R: smithay::backend::renderer::Renderer,
    <R as smithay::backend::renderer::Renderer>::TextureId: 'static,
    E: smithay::backend::renderer::element::Element
        + smithay::backend::renderer::element::RenderElement<R>,
    R: ImportAll + ImportMem,
{
}

impl<R, E> std::fmt::Debug for OutputRenderElement<R, E>
where
    R: Renderer + ImportAll + ImportMem,
    E: RenderElement<R> + std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Space(arg0) => f.debug_tuple("Space").field(arg0).finish(),
            Self::Window(arg0) => f.debug_tuple("Window").field(arg0).finish(),
            Self::Custom(arg0) => f.debug_tuple("Custom").field(arg0).finish(),
            Self::SessionLockSurface(arg0) => {
                f.debug_tuple("SessionLockSurface").field(arg0).finish()
            }
            Self::SessionLockBackground(arg0) => {
                f.debug_tuple("SessionLockBackground").field(arg0).finish()
            }
        }
    }
}

#[derive(Debug)]
pub(crate) struct SurfaceDmabufFeedback<'a> {
    pub render_feedback: &'a DmabufFeedback,
    pub scanout_feedback: &'a DmabufFeedback,
}

impl InnerState {
    pub(crate) fn make_output_elements<R>(
        &self,
        renderer: &mut R,
        output: &Output,
        additional_elements: Vec<CustomRenderElement<R>>,
    ) -> (
        Vec<OutputRenderElement<R, WindowRenderElement<R>>>,
        [f32; 4],
    )
    where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + 'static,
    {
        use smithay::backend::renderer::element::surface::render_elements_from_surface_tree;
        use smithay::backend::renderer::element::Kind;

        let mut elements = additional_elements
            .into_iter()
            .map(OutputRenderElement::from)
            .collect::<Vec<_>>();

        'body: {
            use crate::session_lock::SessionLockState;
            match self.session_lock_data.get_lock_surface(output) {
                SessionLockState::NotLocked => {}
                SessionLockState::Locked(output_assoc) => {
                    // If the session is locked, hide outputs by solid background and show a lock screen if exists.
                    // Note that a lock screen may not exist, for example, if it is not yet provided or the lock client is killed.

                    let output_scale =
                        smithay::utils::Scale::from(output.current_scale().fractional_scale());

                    if let Some(lock_surface) = &output_assoc.lock_surface {
                        elements.extend(
                            render_elements_from_surface_tree(
                                renderer,
                                lock_surface.wl_surface(),
                                (0, 0),
                                output_scale,
                                1.,
                                Kind::Unspecified,
                            )
                            .into_iter()
                            .map(OutputRenderElement::SessionLockSurface),
                        );
                    }

                    elements.push(OutputRenderElement::SessionLockBackground(
                        SolidColorRenderElement::from_buffer(
                            &output_assoc.background,
                            (0, 0),
                            output_scale,
                            1.,
                            Kind::Unspecified,
                        ),
                    ));

                    #[cfg(not(feature = "debug_session_lock_client_dead"))]
                    break 'body;
                }
            }

            let space_elements = smithay::desktop::space::space_render_elements(
                renderer,
                [&self.space],
                output,
                1.0,
            )
            .expect("output without mode?");
            elements.extend(space_elements.into_iter().map(OutputRenderElement::Space));
        }

        (elements, CLEAR_COLOR)
    }

    pub(crate) fn post_repaint(
        &self,
        output: &smithay::output::Output,
        render_element_states: &RenderElementStates,
        dmabuf_feedback: Option<SurfaceDmabufFeedback<'_>>,
        time: Duration,
    ) {
        use smithay::backend::renderer::element::default_primary_scanout_output_compare;
        use smithay::backend::renderer::element::utils::select_dmabuf_feedback;
        use smithay::desktop::utils::{
            send_dmabuf_feedback_surface_tree, send_frames_surface_tree,
            surface_primary_scanout_output, update_surface_primary_scanout_output,
        };
        use smithay::wayland::compositor::{with_surface_tree_downward, TraversalAction};
        use smithay::wayland::fractional_scale::with_fractional_scale;

        let throttle = Some(Duration::from_secs(1));

        use crate::session_lock::SessionLockState;
        match self.session_lock_data.get_lock_surface(output) {
            SessionLockState::NotLocked => {}
            SessionLockState::Locked(output_assoc) => {
                if let Some(lock_surface) = &output_assoc.lock_surface {
                    with_surface_tree_downward(
                        lock_surface.wl_surface(),
                        (),
                        |_, _, _| TraversalAction::DoChildren(()),
                        |surface, states, _| {
                            let primary_scanout_output = update_surface_primary_scanout_output(
                                surface,
                                output,
                                states,
                                render_element_states,
                                default_primary_scanout_output_compare,
                            );
                            if let Some(output) = primary_scanout_output {
                                with_fractional_scale(states, |fraction_scale| {
                                    fraction_scale.set_preferred_scale(
                                        output.current_scale().fractional_scale(),
                                    );
                                });
                            }
                        },
                        |_, _, _| true,
                    );
                    send_frames_surface_tree(
                        lock_surface.wl_surface(),
                        output,
                        time,
                        throttle,
                        surface_primary_scanout_output,
                    );
                    if let Some(dmabuf_feedback) = dmabuf_feedback {
                        send_dmabuf_feedback_surface_tree(
                            lock_surface.wl_surface(),
                            output,
                            |_, _| Some(output.clone()),
                            |surface, _| {
                                select_dmabuf_feedback(
                                    surface,
                                    render_element_states,
                                    dmabuf_feedback.render_feedback,
                                    dmabuf_feedback.scanout_feedback,
                                )
                            },
                        );
                    }
                }

                #[cfg(not(feature = "debug_session_lock_client_dead"))]
                return;
            }
        }

        for window in self.space.elements() {
            window.smithay_window().with_surfaces(|surface, states| {
                let primary_scanout_output = update_surface_primary_scanout_output(
                    surface,
                    output,
                    states,
                    render_element_states,
                    default_primary_scanout_output_compare,
                );
                if let Some(output) = primary_scanout_output {
                    with_fractional_scale(states, |fraction_scale| {
                        fraction_scale
                            .set_preferred_scale(output.current_scale().fractional_scale());
                    });
                }
            });

            if self.space.outputs_for_element(window).contains(output) {
                window.smithay_window().send_frame(
                    output,
                    time,
                    throttle,
                    surface_primary_scanout_output,
                );
                if let Some(dmabuf_feedback) = &dmabuf_feedback {
                    window.smithay_window().send_dmabuf_feedback(
                        output,
                        surface_primary_scanout_output,
                        |surface, _| {
                            select_dmabuf_feedback(
                                surface,
                                render_element_states,
                                dmabuf_feedback.render_feedback,
                                dmabuf_feedback.scanout_feedback,
                            )
                        },
                    );
                }
            }
        }

        let map = smithay::desktop::layer_map_for_output(output);
        for layer_surface in map.layers() {
            layer_surface.with_surfaces(|surface, states| {
                let primary_scanout_output = update_surface_primary_scanout_output(
                    surface,
                    output,
                    states,
                    render_element_states,
                    default_primary_scanout_output_compare,
                );
                if let Some(output) = primary_scanout_output {
                    with_fractional_scale(states, |fraction_scale| {
                        fraction_scale
                            .set_preferred_scale(output.current_scale().fractional_scale());
                    });
                }
            });

            layer_surface.send_frame(output, time, throttle, surface_primary_scanout_output);
            if let Some(dmabuf_feedback) = &dmabuf_feedback {
                layer_surface.send_dmabuf_feedback(
                    output,
                    surface_primary_scanout_output,
                    |surface, _| {
                        select_dmabuf_feedback(
                            surface,
                            render_element_states,
                            dmabuf_feedback.render_feedback,
                            dmabuf_feedback.scanout_feedback,
                        )
                    },
                );
            }
        }
    }

    pub(crate) fn take_presentation_feedback(
        &self,
        output: &smithay::output::Output,
        render_element_states: &RenderElementStates,
    ) -> smithay::desktop::utils::OutputPresentationFeedback {
        use smithay::desktop::utils::{
            surface_presentation_feedback_flags_from_states, surface_primary_scanout_output,
            take_presentation_feedback_surface_tree, OutputPresentationFeedback,
        };

        let mut output_presentation_feedback = OutputPresentationFeedback::new(output);

        'body: {
            use crate::session_lock::SessionLockState;
            match self.session_lock_data.get_lock_surface(output) {
                SessionLockState::NotLocked => {}
                SessionLockState::Locked(output_assoc) => {
                    if let Some(lock_surface) = &output_assoc.lock_surface {
                        take_presentation_feedback_surface_tree(
                            lock_surface.wl_surface(),
                            &mut output_presentation_feedback,
                            surface_primary_scanout_output,
                            |surface, _| {
                                surface_presentation_feedback_flags_from_states(
                                    surface,
                                    render_element_states,
                                )
                            },
                        );
                    }

                    #[cfg(not(feature = "debug_session_lock_client_dead"))]
                    break 'body;
                }
            }

            for window in self.space.elements() {
                if self.space.outputs_for_element(window).contains(output) {
                    window.smithay_window().take_presentation_feedback(
                        &mut output_presentation_feedback,
                        surface_primary_scanout_output,
                        |surface, _| {
                            surface_presentation_feedback_flags_from_states(
                                surface,
                                render_element_states,
                            )
                        },
                    );
                }
            }

            let map = smithay::desktop::layer_map_for_output(output);
            for layer_surface in map.layers() {
                layer_surface.take_presentation_feedback(
                    &mut output_presentation_feedback,
                    surface_primary_scanout_output,
                    |surface, _| {
                        surface_presentation_feedback_flags_from_states(
                            surface,
                            render_element_states,
                        )
                    },
                );
            }
        }

        output_presentation_feedback
    }
}
