use crate::pointer::{PointerRenderElement, CLEAR_COLOR};
use crate::state::InnerState;
use crate::view::window::WindowRenderElement;
use smithay::backend::renderer::element::solid::SolidColorRenderElement;
use smithay::backend::renderer::element::surface::WaylandSurfaceRenderElement;
use smithay::backend::renderer::element::{RenderElement, Wrap};
use smithay::backend::renderer::{ImportAll, ImportMem, Renderer};
use smithay::desktop::space::SpaceRenderElements;
use smithay::output::Output;

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

pub(crate) fn output_elements<R>(
    // TODO: Make it a method.
    this: &InnerState,
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

    use crate::session_lock::SessionLockState;
    match this.session_lock_data.get_lock_surface(output) {
        SessionLockState::NotLocked => {}
        SessionLockState::Locked(output_assoc)
        | SessionLockState::LockedButClientGone(output_assoc) => {
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
        }
    }

    let space_elements =
        smithay::desktop::space::space_render_elements(renderer, [&this.space], output, 1.0)
            .expect("output without mode?");
    elements.extend(space_elements.into_iter().map(OutputRenderElement::Space));

    (elements, CLEAR_COLOR)
}
