use super::TransformBehavior;
use sabiniwm_base::smithay_ext::utils::TransformTriple;
use smithay::reexports::wayland_server;
use smithay::utils::{Buffer as BufferCoord, Logical, Rectangle, Size, Transform};

pub(super) fn bytes_per_pixel(format: wayland_server::protocol::wl_shm::Format) -> u32 {
    use wayland_server::protocol::wl_shm::Format;

    match format {
        Format::Argb8888 => 4,
        Format::Xrgb8888 => 4,
        _ => {
            unimplemented!();
        }
    }
}

pub(super) fn calc_buffer_size(
    transform_behavior: TransformBehavior,
    tt: &TransformTriple,
    geometry_logical: &Rectangle<i32, Logical>,
) -> Size<i32, BufferCoord> {
    match transform_behavior {
        TransformBehavior::NoTransform => geometry_logical.size.to_buffer(1, Transform::Normal),
        TransformBehavior::Scale => tt
            .map_rect_logical_to_mid(geometry_logical)
            .size
            .to_logical(1)
            .to_buffer(1, Transform::Normal),
        // We can't determine terminal transform without knowledge of backend. But the options are
        // `Transform::Normal` or `Transform::Flipped180`, and it doesn't affect size.
        TransformBehavior::ScaleActual | TransformBehavior::ScaleActualTerminal => tt
            .map_rect_logical_to_physical(geometry_logical)
            .size
            .to_logical(1)
            .to_buffer(1, Transform::Normal),
    }
}
