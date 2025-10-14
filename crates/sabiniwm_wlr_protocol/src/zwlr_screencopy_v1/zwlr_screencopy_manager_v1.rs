use crate::zwlr_screencopy_v1::global::ScreencopyGlobalData;
use crate::zwlr_screencopy_v1::zwlr_screencopy_frame_v1::{
    ScreencopyFrameData, ScreencopyFrameDataState, ScreencopyFrameError,
};
use crate::zwlr_screencopy_v1::{ScreencopyDispatcher, ScreencopyHandler};
use sabiniwm_base::smithay_ext::utils::{OutputExt, TransformTriple};
use smithay::output::Output;
use smithay::reexports::wayland_protocols_wlr::screencopy::v1::server::{
    zwlr_screencopy_frame_v1, zwlr_screencopy_manager_v1,
};
use smithay::reexports::wayland_server::{
    Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, Resource,
};
use smithay::utils::{Logical, Rectangle};

/// Crate internal data. Public only for `delegate_zwlr_screencopy_v1!`.
///
/// `UserData` of zwlr_screencopy_manager_v1
#[doc(hidden)]
pub struct ScreencopyManagerData;

impl<D> Dispatch<zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1, ScreencopyManagerData, D>
    for ScreencopyDispatcher
where
    D: GlobalDispatch<zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1, ScreencopyGlobalData>
        + Dispatch<zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1, ScreencopyManagerData>
        + Dispatch<zwlr_screencopy_frame_v1::ZwlrScreencopyFrameV1, ScreencopyFrameData>
        + ScreencopyHandler
        + 'static,
{
    fn request(
        _state: &mut D,
        _client: &Client,
        resource: &zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1,
        request: zwlr_screencopy_manager_v1::Request,
        _data: &ScreencopyManagerData,
        _display_handle: &DisplayHandle,
        data_init: &mut DataInit<'_, D>,
    ) {
        let (resource_frame_id, should_paint_cursors, wl_output, rect) = match request {
            zwlr_screencopy_manager_v1::Request::Destroy => return,
            zwlr_screencopy_manager_v1::Request::CaptureOutput {
                frame: resource_frame_id,
                overlay_cursor,
                output: wl_output,
            } => (resource_frame_id, overlay_cursor != 0, wl_output, None),
            zwlr_screencopy_manager_v1::Request::CaptureOutputRegion {
                frame: resource_frame_id,
                overlay_cursor,
                output: wl_output,
                x,
                y,
                width,
                height,
            } => {
                let rect: Rectangle<i32, Logical> =
                    Rectangle::new((x, y).into(), (width, height).into());
                (
                    resource_frame_id,
                    overlay_cursor != 0,
                    wl_output,
                    Some(rect),
                )
            }
            _ => unreachable!(),
        };

        let mut data_frame = ScreencopyFrameData::new();
        let resource_frame = data_init.init(resource_frame_id, data_frame.clone());

        let Some(output) = Output::from_resource(&wl_output) else {
            info!("zwlr_screencopy_manager_v1: Output for WlOutput not found");
            ScreencopyFrameError::new_no_error_code("Output for WlOutput not found")
                .send_error_failed(&resource_frame, &mut data_frame);
            return;
        };

        let transform_triple = TransformTriple::new_without_terminal_correction(&output)
            .unwrap(/* Output::current_mode() is set */);
        let geometry_logical = if let Some(rect) = rect {
            rect
        } else {
            Rectangle::new((0, 0).into(), output.current_logical_size())
        };
        let size_buffer = super::internal::calc_buffer_size(
            D::TRANSFORM_BEHAVIOR,
            &transform_triple,
            &geometry_logical,
        );
        let (w, h) = (size_buffer.w as u32, size_buffer.h as u32);

        resource_frame.buffer(
            D::SUPPORTED_FORMATS.shm_format,
            w,
            h,
            w * super::internal::bytes_per_pixel(D::SUPPORTED_FORMATS.shm_format),
        );

        if resource.version() >= 3 {
            for dmabuf_format in D::SUPPORTED_FORMATS.dmabuf_formats {
                resource_frame.linux_dmabuf(*dmabuf_format as u32, w, h);
            }

            resource_frame.buffer_done();
        }

        data_frame
            .transition_state(ScreencopyFrameDataState::WaitingBuffer {
                wl_output,
                geometry_logical,
                transform_triple,
                should_paint_cursors,
            })
            .unwrap();
    }
}
