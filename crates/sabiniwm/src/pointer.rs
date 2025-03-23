#![allow(clippy::too_many_arguments)]

use smithay::backend::renderer::element::memory::{
    MemoryRenderBuffer, MemoryRenderBufferRenderElement,
};
use smithay::backend::renderer::element::surface::WaylandSurfaceRenderElement;
use smithay::backend::renderer::element::{AsRenderElements, Kind};
use smithay::backend::renderer::{Color32F, ImportAll, ImportMem, Renderer, Texture};
use smithay::input::pointer::CursorImageStatus;
use smithay::utils::{Physical, Point, Scale};

pub static CLEAR_COLOR: Color32F = Color32F::new(0.1, 0.1, 0.1, 0.0);
pub static CLEAR_COLOR_FULLSCREEN: Color32F = Color32F::new(0.0, 0.0, 0.0, 0.0);

pub struct PointerElement {
    buffer: Option<MemoryRenderBuffer>,
    status: CursorImageStatus,
}

impl Default for PointerElement {
    fn default() -> Self {
        Self {
            buffer: Default::default(),
            status: CursorImageStatus::default_named(),
        }
    }
}

impl PointerElement {
    pub fn set_buffer(&mut self, buffer: MemoryRenderBuffer) {
        self.buffer = Some(buffer);
    }

    pub fn set_status(&mut self, status: CursorImageStatus) {
        self.status = status;
    }
}

#[derive(derive_more::From)]
#[thin_delegate::register]
pub enum PointerRenderElement<R>
where
    R: Renderer,
{
    Surface(WaylandSurfaceRenderElement<R>),
    Memory(MemoryRenderBufferRenderElement<R>),
}

#[thin_delegate::fill_delegate(external_trait_def = crate::external_trait_def::smithay::backend::renderer::element)]
impl<R> smithay::backend::renderer::element::Element for PointerRenderElement<R>
where
    R: smithay::backend::renderer::Renderer,
    R::TextureId: 'static,
    R: ImportAll + ImportMem,
{
}

#[thin_delegate::fill_delegate(external_trait_def = crate::external_trait_def::smithay::backend::renderer::element)]
impl<R> smithay::backend::renderer::element::RenderElement<R> for PointerRenderElement<R>
where
    R: smithay::backend::renderer::Renderer,
    R::TextureId: 'static,
    R: ImportAll + ImportMem,
{
}

impl<R> std::fmt::Debug for PointerRenderElement<R>
where
    R: Renderer,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Surface(arg0) => f.debug_tuple("Surface").field(arg0).finish(),
            Self::Memory(arg0) => f.debug_tuple("Memory").field(arg0).finish(),
        }
    }
}

impl<T, R> AsRenderElements<R> for PointerElement
where
    T: Texture + Clone + Send + 'static,
    R: Renderer<TextureId = T> + ImportAll + ImportMem,
{
    type RenderElement = PointerRenderElement<R>;

    fn render_elements<E>(
        &self,
        renderer: &mut R,
        location: Point<i32, Physical>,
        scale: Scale<f64>,
        alpha: f32,
    ) -> Vec<E>
    where
        E: From<PointerRenderElement<R>>,
    {
        match &self.status {
            CursorImageStatus::Hidden => vec![],
            // Always render `Default` for a named shape.
            CursorImageStatus::Named(_) => {
                if let Some(buffer) = self.buffer.as_ref() {
                    vec![PointerRenderElement::<R>::from(
                        MemoryRenderBufferRenderElement::from_buffer(
                            renderer,
                            location.to_f64(),
                            buffer,
                            None,
                            None,
                            None,
                            Kind::Cursor,
                        )
                        .expect("Lost system pointer buffer"),
                    )
                    .into()]
                } else {
                    vec![]
                }
            }
            CursorImageStatus::Surface(surface) => {
                let elements: Vec<PointerRenderElement<R>> =
                    smithay::backend::renderer::element::surface::render_elements_from_surface_tree(
                        renderer,
                        surface,
                        location,
                        scale,
                        alpha,
                        Kind::Cursor,
                    );
                elements.into_iter().map(E::from).collect()
            }
        }
    }
}
