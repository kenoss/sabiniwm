use crate::SabiniwmState;
use sabiniwm_base::smithay_ext::utils::{CoordSystem, FixedTransform, TransformTriple};
use sabiniwm_wlr_protocol::zwlr_screencopy_v1::TransformBehavior;
use smithay::backend::allocator::Fourcc;
use smithay::backend::allocator::dmabuf::Dmabuf;
use smithay::backend::renderer::element::RenderElement;
use smithay::backend::renderer::gles::GlesTexture;
use smithay::backend::renderer::sync::SyncPoint;
use smithay::backend::renderer::{Color32F, ExportMem, Frame, Renderer, RendererSuper};
use smithay::utils::{Buffer as BufferCoord, Rectangle, Scale, Size, Transform};

fn render_to_framebuffer<'fb, 'buffer, R>(
    renderer: &'fb mut R,
    tt: TransformTriple,
    elements: impl DoubleEndedIterator<Item = impl RenderElement<R>>,
    fb: &'fb mut <R as RendererSuper>::Framebuffer<'buffer>,
    size_buffer: Size<i32, BufferCoord>,
    clear_color: Color32F,
) -> Result<SyncPoint, <R as RendererSuper>::Error>
where
    'buffer: 'fb,
    R: Renderer,
{
    // Just change `Kind` to `Physical` as destination of rendering is `Physical`.
    let size_buffer = size_buffer.to_logical(1, Transform::Normal).to_physical(1);

    let damage = [Rectangle::from_size(tt.size_mid())];
    let damage = &damage;

    let scale = Scale::from(tt.scale.fractional_scale());
    // #mythery-invert-for-draw
    //
    // `Renderer::render()` requires inverted transform. kenoss is not sure why.
    //
    // It might be related to #Transform-transform_rect_in-wrong. The current implementation of
    // `Transform::_90` works as if it is `Transform::_270`. (Unlike `FixedTransform`.)
    //
    // TODO: Investigate it.
    let transform = tt.transform.invert();
    let transform = Transform::from(transform);

    let mut frame = renderer.render(fb, size_buffer, transform)?;

    frame.clear(clear_color, damage)?;

    for element in elements.rev() {
        let src = element.src();
        let dst = element.geometry(scale);
        element.draw(&mut frame, src, dst, damage, &[])?;
    }

    frame.finish()
}

fn render_to_texture_mapping<R>(
    renderer: &mut R,
    tt: TransformTriple,
    elements: impl DoubleEndedIterator<Item = impl RenderElement<R>>,
    fourcc: Fourcc,
    size_buffer: Size<i32, BufferCoord>,
    clear_color: Color32F,
) -> Result<<R as ExportMem>::TextureMapping, <R as RendererSuper>::Error>
where
    R: Renderer
        + ExportMem
        + smithay::backend::renderer::Bind<GlesTexture>
        + smithay::backend::renderer::Offscreen<GlesTexture>,
    <R as RendererSuper>::TextureId: Clone,
{
    use smithay::backend::renderer::Offscreen;

    let mut texture = Offscreen::create_buffer(renderer, fourcc, size_buffer)?;
    let mut target = renderer.bind(&mut texture)?;

    let sync_point = render_to_framebuffer(
        renderer,
        tt,
        elements,
        &mut target,
        size_buffer,
        clear_color,
    )?;
    while matches!(
        sync_point.wait(),
        Err(smithay::backend::renderer::sync::Interrupted)
    ) {}

    let tex_mapping =
        renderer.copy_framebuffer(&target, Rectangle::from_size(size_buffer), fourcc)?;
    Ok(tex_mapping)
}

fn render_to_shm<R>(
    renderer: &mut R,
    tt: TransformTriple,
    elements: impl DoubleEndedIterator<Item = impl RenderElement<R>>,
    buffer_contents: (*mut u8, usize, smithay::wayland::shm::BufferData),
    size_buffer: Size<i32, BufferCoord>,
    clear_color: Color32F,
) -> Result<(), <R as RendererSuper>::Error>
where
    R: Renderer
        + ExportMem
        + smithay::backend::renderer::Bind<GlesTexture>
        + smithay::backend::renderer::Offscreen<GlesTexture>,
    <R as RendererSuper>::TextureId: Clone,
{
    let (shm_buffer, shm_len, buffer_data) = buffer_contents;

    let fourcc = smithay::wayland::shm::shm_format_to_fourcc(buffer_data.format).unwrap();
    let tex_mapping =
        render_to_texture_mapping(renderer, tt, elements, fourcc, size_buffer, clear_color)?;
    let bytes = renderer.map_texture(&tex_mapping)?;

    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), shm_buffer.cast(), shm_len);
    }

    Ok(())
}

pub fn render_to_dmabuf<R>(
    renderer: &mut R,
    tt: TransformTriple,
    elements: impl DoubleEndedIterator<Item = impl RenderElement<R>>,
    dmabuf: &Dmabuf,
    size_buffer: Size<i32, BufferCoord>,
    clear_color: Color32F,
) -> Result<(), <R as RendererSuper>::Error>
where
    R: Renderer + smithay::backend::renderer::Bind<Dmabuf>,
    <R as RendererSuper>::TextureId: Clone,
{
    let mut dmabuf = dmabuf.clone();
    let mut target = renderer.bind(&mut dmabuf)?;

    let sync_point = render_to_framebuffer(
        renderer,
        tt,
        elements,
        &mut target,
        size_buffer,
        clear_color,
    )?;
    while matches!(
        sync_point.wait(),
        Err(smithay::backend::renderer::sync::Interrupted)
    ) {}

    Ok(())
}

/// Render to `ScreencopyFrameRequest`
///
/// ## Arguments
///
/// - `coord_system`: `CoordSystem` of `GlesTexture` of `R`.
// See note/issue-transform.md and note/screencopy.md.
pub fn render_to_screencopy_frame_request<R>(
    renderer: &mut R,
    output: &smithay::output::Output,
    coord_system: CoordSystem,
    elements: impl DoubleEndedIterator<Item = impl RenderElement<R>>,
    frame_request: sabiniwm_wlr_protocol::zwlr_screencopy_v1::ScreencopyFrameRequest,
    clear_color: Color32F,
) where
    R: Renderer
        + ExportMem
        + smithay::backend::renderer::Bind<GlesTexture>
        + smithay::backend::renderer::Offscreen<GlesTexture>
        + smithay::backend::renderer::Bind<Dmabuf>,
    <R as RendererSuper>::TextureId: Clone,
{
    #![allow(clippy::clone_on_copy)]

    use sabiniwm_wlr_protocol::zwlr_screencopy_v1::ScreencopyHandler;
    use smithay::backend::renderer::element::utils::{Relocate, RelocateRenderElement};

    let buffer = frame_request.buffer();
    let size_buffer = frame_request.size_buffer().clone();

    let tt = TransformTriple::new_without_terminal_correction(output)
        .unwrap(/* Output::current_mode() is set */);
    let tt = match SabiniwmState::TRANSFORM_BEHAVIOR {
        TransformBehavior::NoTransform => TransformTriple {
            size: tt.size,
            scale: smithay::output::Scale::Integer(1),
            transform: FixedTransform::Normal,
        },
        TransformBehavior::Scale => TransformTriple {
            size: tt.size,
            scale: tt.scale,
            transform: FixedTransform::Normal,
        },
        TransformBehavior::ScaleActual => {
            TransformTriple::new_with_terminal_correction(output, coord_system)
                .unwrap(/* Output::current_mode() is set */)
        }
        TransformBehavior::ScaleActualTerminal => tt,
    };

    let geometry_mid = tt.map_rect_logical_to_mid(frame_request.geometry_logical());
    let relocate_loc = (-geometry_mid.loc.x, -geometry_mid.loc.y);
    let elements =
        elements.map(|e| RelocateRenderElement::from_element(e, relocate_loc, Relocate::Relative));

    // Question: Can a client update `WlBuffer` after the handling of zwlr_screencopy_frame_v1?
    //
    // Here we assume that it is unchanged. We might check it is unchanged.
    let res = match smithay::backend::renderer::buffer_type(buffer) {
        Some(smithay::backend::renderer::BufferType::Shm) => {
            let res = smithay::wayland::shm::with_buffer_contents_mut(
                buffer,
                |shm_buffer, shm_len, buffer_data| {
                    render_to_shm(
                        renderer,
                        tt,
                        elements,
                        (shm_buffer, shm_len, buffer_data),
                        size_buffer,
                        clear_color,
                    )
                },
            );
            match res {
                Ok(x) => x,
                Err(e) => {
                    warn!("failed to render for screencast: {e:?}");
                    frame_request.send_failed();
                    return;
                }
            }
        }
        Some(smithay::backend::renderer::BufferType::Dma) => {
            let dmabuf = smithay::wayland::dmabuf::get_dmabuf(buffer)
                .unwrap(/* smithay::backend::renderer::buffer_type() is already checked */);
            render_to_dmabuf(renderer, tt, elements, dmabuf, size_buffer, clear_color)
        }
        // `sabiniwm_wlr_protocol::zwlr_screencopy_v1` ensures that the buffer is shm or dmabuf.
        _ => unreachable!(),
    };
    match res {
        Ok(()) => {
            frame_request.send_ready();
        }
        Err(e) => {
            warn!("failed to render for screencast: {e:?}");
            frame_request.send_failed();
        }
    }
}

mod handler {
    use super::*;
    use crate::state::SabiniwmState;
    use smithay::reexports::wayland_server::protocol::wl_shm;

    impl sabiniwm_wlr_protocol::zwlr_screencopy_v1::ScreencopyHandler for SabiniwmState {
        // See note/screencopy.md.
        const TRANSFORM_BEHAVIOR: sabiniwm_wlr_protocol::zwlr_screencopy_v1::TransformBehavior =
            TransformBehavior::ScaleActualTerminal;

        const SUPPORTED_FORMATS: sabiniwm_wlr_protocol::zwlr_screencopy_v1::SupportedFormats =
            sabiniwm_wlr_protocol::zwlr_screencopy_v1::SupportedFormats {
                shm_format: wl_shm::Format::Argb8888,
                dmabuf_formats: &[
                    smithay::backend::allocator::Fourcc::Abgr8888,
                    smithay::backend::allocator::Fourcc::Argb8888,
                    smithay::backend::allocator::Fourcc::Xbgr8888,
                    smithay::backend::allocator::Fourcc::Xrgb8888,
                ],
            };

        fn on_frame_request(
            &self,
            frame_request: sabiniwm_wlr_protocol::zwlr_screencopy_v1::ScreencopyFrameRequest,
            mut output: smithay::output::Output,
            _should_wait_damage: bool,
        ) {
            use sabiniwm_wlr_protocol::zwlr_screencopy_v1::OutputExtForScreencopy;

            // Attach `ScreencopyFrameRequest` to `Output`. This will be handled by
            // `OutputExtForScreencopy::take_screencopy_frame_requests()` when the next render.
            output.add_screencopy_frame_request(frame_request);

            // Don't handle `should_wait_damage` as sabiniwm currently doesn't pause rendering.
        }
    }

    sabiniwm_wlr_protocol::delegate_zwlr_screencopy_v1!(SabiniwmState);
}
