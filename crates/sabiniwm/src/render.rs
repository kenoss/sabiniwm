use crate::pointer::{PointerRenderElement, CLEAR_COLOR};
use crate::view::window::WindowRenderElement;
use smithay::backend::renderer::element::surface::WaylandSurfaceRenderElement;
use smithay::backend::renderer::element::{RenderElement, Wrap};
use smithay::backend::renderer::{ImportAll, ImportMem, Renderer};
use smithay::desktop::space::{Space, SpaceRenderElements};
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
        }
    }
}

pub fn output_elements<R>(
    renderer: &mut R,
    output: &Output,
    space: &Space<crate::view::window::Window>,
    custom_elements: Vec<CustomRenderElement<R>>,
) -> (
    Vec<OutputRenderElement<R, WindowRenderElement<R>>>,
    [f32; 4],
)
where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Clone + 'static,
{
    let mut output_render_elements = custom_elements
        .into_iter()
        .map(OutputRenderElement::from)
        .collect::<Vec<_>>();

    let space_elements =
        smithay::desktop::space::space_render_elements(renderer, [space], output, 1.0)
            .expect("output without mode?");
    output_render_elements.extend(space_elements.into_iter().map(OutputRenderElement::Space));

    (output_render_elements, CLEAR_COLOR)
}
