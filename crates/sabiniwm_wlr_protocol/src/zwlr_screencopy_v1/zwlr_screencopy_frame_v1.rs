use crate::zwlr_screencopy_v1::global::ScreencopyGlobalData;
use crate::zwlr_screencopy_v1::{ScreencopyDispatcher, ScreencopyHandler, ScreencopyManagerData};
use sabiniwm_base::smithay_ext::utils::TransformTriple;
use sabiniwm_base::smithay_ext::wayland::shm::BufferDataExt;
use smithay::backend::allocator::Buffer;
use smithay::output::Output;
use smithay::reexports::wayland_protocols_wlr::screencopy::v1::server::{
    zwlr_screencopy_frame_v1, zwlr_screencopy_manager_v1,
};
use smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer;
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::reexports::wayland_server::{
    Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, Resource,
};
use smithay::utils::{Buffer as BufferCoord, Logical, Rectangle, Size};
use std::borrow::Cow;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex};

pub(super) struct ScreencopyFrameError {
    /// If non null, it will be reported to a client by
    /// [`zwlr_screencopy_frame_v1::error`](https://wayland.app/protocols/wlr-screencopy-unstable-v1#zwlr_screencopy_frame_v1:enum:error).
    code: Option<zwlr_screencopy_frame_v1::Error>,
    /// Used only to carry an error message for [`ScreencopyFrameError::msg()`].
    msg: Cow<'static, str>,
}

impl ScreencopyFrameError {
    pub fn new(code: zwlr_screencopy_frame_v1::Error, msg: impl Into<Cow<'static, str>>) -> Self {
        Self {
            code: Some(code),
            msg: msg.into(),
        }
    }

    pub fn new_no_error_code(msg: impl Into<Cow<'static, str>>) -> Self {
        Self {
            code: None,
            msg: msg.into(),
        }
    }

    pub fn msg(&self) -> &str {
        &self.msg
    }

    #[inline]
    pub fn send_error_failed(
        self,
        resource: &zwlr_screencopy_frame_v1::ZwlrScreencopyFrameV1,
        data: &mut ScreencopyFrameData,
    ) {
        self.send_error_failed_non_mut(resource, data);
    }

    pub fn send_error_failed_non_mut(
        self,
        resource: &zwlr_screencopy_frame_v1::ZwlrScreencopyFrameV1,
        data: &ScreencopyFrameData,
    ) {
        if data.is_terminal_state() {
            return;
        }

        data.transition_state_non_mut(ScreencopyFrameDataState::Failed)
            .unwrap();

        if let Some(code) = self.code {
            resource.post_error(code, self.msg);
        }
        resource.failed();
    }
}

/// Crate internal data. Public only for `delegate_zwlr_screencopy_v1!`.
///
/// `UserData` of zwlr_screencopy_frame_v1
#[doc(hidden)]
#[derive(Clone)]
pub struct ScreencopyFrameData(Arc<Mutex<ScreencopyFrameDataState>>);

#[cfg_attr(doc, aquamarine::aquamarine)]
/// ```mermaid
/// graph LR
///     Initializing --> WaitingBuffer
///     Initializing --> Failed
///     WaitingBuffer --> ReceivingBuffer
///     WaitingBuffer --> Destroyed
///     ReceivingBuffer --> WaitingRender
///     WaitingRender --> Ready
///     WaitingRender --> Failed
///     WaitingRender --> Destroyed
///     WaitingRender --> Dropped
/// ```
#[derive(enum_kinds::EnumKind)]
#[enum_kind(ScreencopyFrameDataStateKind)]
pub(super) enum ScreencopyFrameDataState {
    Initializing,
    WaitingBuffer {
        wl_output: WlOutput,
        geometry_logical: Rectangle<i32, Logical>,
        transform_triple: TransformTriple,
        should_paint_cursors: bool,
    },
    ReceivingBuffer,
    WaitingRender,
    Ready,
    Failed,
    Destroyed,
    Dropped,
}

impl ScreencopyFrameData {
    pub(super) fn new() -> Self {
        Self(Arc::new(Mutex::new(ScreencopyFrameDataState::Initializing)))
    }

    fn assert_transition(
        old: ScreencopyFrameDataStateKind,
        new: ScreencopyFrameDataStateKind,
    ) -> Result<(), String> {
        use ScreencopyFrameDataStateKind as K;

        match (old, new) {
            // TODO: Ends with terminal states Ready/Failed.
            (K::Initializing, K::WaitingBuffer)
            | (K::Initializing, K::Failed)
            | (K::WaitingBuffer, K::ReceivingBuffer)
            | (K::WaitingBuffer, K::Destroyed)
            | (K::ReceivingBuffer, K::WaitingRender)
            | (K::WaitingRender, K::Ready)
            | (K::WaitingRender, K::Failed)
            | (K::WaitingRender, K::Destroyed)
            | (K::WaitingRender, K::Dropped) => {}
            _ => return Err(format!("invalid state transition: {old:?} -> {new:?}")),
        }

        Ok(())
    }

    pub(super) fn transition_state(
        &mut self,
        new: ScreencopyFrameDataState,
    ) -> Result<ScreencopyFrameDataState, String> {
        self.transition_state_non_mut(new)
    }

    // We need it as `Dispatch::request()` passes an argument as non mut ref.
    pub(super) fn transition_state_non_mut(
        &self,
        mut new: ScreencopyFrameDataState,
    ) -> Result<ScreencopyFrameDataState, String> {
        use ScreencopyFrameDataStateKind as K;

        let mut inner = self.0.lock().unwrap();
        Self::assert_transition(K::from(inner.deref()), K::from(&new))?;
        std::mem::swap(inner.deref_mut(), &mut new);
        Ok(new)
    }

    fn state_kind(&self) -> ScreencopyFrameDataStateKind {
        self.0.lock().unwrap().deref().into()
    }

    pub(super) fn is_terminal_state(&self) -> bool {
        [
            ScreencopyFrameDataStateKind::Ready,
            ScreencopyFrameDataStateKind::Failed,
            ScreencopyFrameDataStateKind::Destroyed,
            ScreencopyFrameDataStateKind::Dropped,
        ]
        .contains(&self.state_kind())
    }
}

impl<D> Dispatch<zwlr_screencopy_frame_v1::ZwlrScreencopyFrameV1, ScreencopyFrameData, D>
    for ScreencopyDispatcher
where
    D: GlobalDispatch<zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1, ScreencopyGlobalData>
        + Dispatch<zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1, ScreencopyManagerData>
        + Dispatch<zwlr_screencopy_frame_v1::ZwlrScreencopyFrameV1, ScreencopyFrameData>
        + ScreencopyHandler
        + 'static,
{
    fn request(
        state: &mut D,
        _client: &Client,
        resource: &zwlr_screencopy_frame_v1::ZwlrScreencopyFrameV1,
        request: zwlr_screencopy_frame_v1::Request,
        data: &ScreencopyFrameData,
        _display_handle: &DisplayHandle,
        _data_init: &mut DataInit<'_, D>,
    ) {
        let res = (|| -> Result<(), ScreencopyFrameError> {
            if data.is_terminal_state() {
                return Ok(());
            }

            let (buffer, should_wait_damage) = match request {
                zwlr_screencopy_frame_v1::Request::Destroy => {
                    data.transition_state_non_mut(ScreencopyFrameDataState::Destroyed)
                        .unwrap();
                    return Ok(());
                }
                zwlr_screencopy_frame_v1::Request::Copy { buffer } => (buffer, false),
                zwlr_screencopy_frame_v1::Request::CopyWithDamage { buffer } => (buffer, true),
                _ => unreachable!(),
            };

            let old = data
                .transition_state_non_mut(ScreencopyFrameDataState::ReceivingBuffer)
                .map_err(|_| ScreencopyFrameError::new_no_error_code("invalid request sequence"))?;
            let ScreencopyFrameDataState::WaitingBuffer {
                wl_output,
                geometry_logical,
                transform_triple,
                should_paint_cursors,
            } = old
            else {
                unreachable!()
            };

            let size_buffer = super::internal::calc_buffer_size(
                D::TRANSFORM_BEHAVIOR,
                &transform_triple,
                &geometry_logical,
            );

            match smithay::backend::renderer::buffer_type(&buffer) {
                Some(smithay::backend::renderer::BufferType::Shm) => {
                    let buffer_data = smithay::wayland::shm::with_buffer_contents(
                        &buffer,
                        |_, _, buffer_data| buffer_data,
                    )
                    .map_err(|_| {
                        ScreencopyFrameError::new(
                            zwlr_screencopy_frame_v1::Error::InvalidBuffer,
                            "can't read buffer data",
                        )
                    })?;

                    if buffer_data.format != D::SUPPORTED_FORMATS.shm_format {
                        return Err(ScreencopyFrameError::new(
                            zwlr_screencopy_frame_v1::Error::InvalidBuffer,
                            "format is not supported",
                        ));
                    }

                    if buffer_data.size() != size_buffer {
                        return Err(ScreencopyFrameError::new(
                            zwlr_screencopy_frame_v1::Error::InvalidBuffer,
                            format!(
                                "size is wrong: got = {:?}, expected = {:?}",
                                buffer_data.size(),
                                size_buffer
                            ),
                        ));
                    }

                    // Safety: Already called once in the resource creation.
                    let bytes_per_pixel =
                        super::internal::bytes_per_pixel(buffer_data.format) as i32;

                    if buffer_data.stride != buffer_data.width * bytes_per_pixel {
                        return Err(ScreencopyFrameError::new(
                            zwlr_screencopy_frame_v1::Error::InvalidBuffer,
                            "stride is wrong",
                        ));
                    }
                }
                Some(smithay::backend::renderer::BufferType::Dma) => {
                    let dmabuf = smithay::wayland::dmabuf::get_dmabuf(&buffer)
                        .unwrap(/* smithay::backend::renderer::buffer_type() already checked */);

                    if dmabuf.size() != size_buffer {
                        return Err(ScreencopyFrameError::new(
                            zwlr_screencopy_frame_v1::Error::InvalidBuffer,
                            format!(
                                "size is wrong: got = {:?}, expected = {:?}",
                                dmabuf.size(),
                                size_buffer
                            ),
                        ));
                    }

                    if !D::SUPPORTED_FORMATS
                        .dmabuf_formats
                        .contains(&dmabuf.format().code)
                    {
                        return Err(ScreencopyFrameError::new(
                            zwlr_screencopy_frame_v1::Error::InvalidBuffer,
                            "invalid format",
                        ));
                    }
                }
                _ => {
                    return Err(ScreencopyFrameError::new(
                        zwlr_screencopy_frame_v1::Error::InvalidBuffer,
                        "invalid buffer type",
                    ));
                }
            };

            let Some(output) = Output::from_resource(&wl_output) else {
                return Err(ScreencopyFrameError::new_no_error_code("output has gone"));
            };

            data.transition_state_non_mut(ScreencopyFrameDataState::WaitingRender)
                .unwrap();

            #[allow(clippy::clone_on_copy)]
            let frame = ScreencopyFrameRequest {
                resource: resource.clone(),
                data: data.clone(),
                geometry_logical,
                transform_triple,
                should_paint_cursors,
                buffer,
                size_buffer,
            };
            state.on_frame_request(frame, output, should_wait_damage);

            Ok(())
        })();
        match res {
            Ok(()) => {}
            Err(e) => {
                warn!("zwlr_screencopy_frame_v1: {}", e.msg());
                e.send_error_failed_non_mut(resource, data);
            }
        }
    }
}

/// Request for screencopy frame
///
/// Drop: If it is dropped without calling [`ScreencopyFrameRequest::send_ready()`] nor
/// [`ScreencopyFrameRequest::send_failed()`], it automatically sends
/// `zwlr_screencopy_frame_v1::failed`. This is intended to be used for drop of
/// [`smithay::output::Output`] that it is attached to. It is recommended to call `send_failed()`
/// explicitly in other cases.
pub struct ScreencopyFrameRequest {
    resource: zwlr_screencopy_frame_v1::ZwlrScreencopyFrameV1,
    data: ScreencopyFrameData,
    geometry_logical: Rectangle<i32, Logical>,
    should_paint_cursors: bool,
    transform_triple: TransformTriple,
    buffer: WlBuffer,
    size_buffer: Size<i32, BufferCoord>,
}

impl Drop for ScreencopyFrameRequest {
    fn drop(&mut self) {
        if self.data.is_terminal_state() {
            return;
        }

        self.data
            .transition_state(ScreencopyFrameDataState::Dropped)
            .unwrap();

        info!("zwlr_screencopy_frame_v1: frame request is dropped. maybe output is dropped?");
        self.resource.failed();
    }
}

impl ScreencopyFrameRequest {
    pub fn geometry_logical(&self) -> &Rectangle<i32, Logical> {
        &self.geometry_logical
    }

    pub fn should_paint_cursors(&self) -> bool {
        self.should_paint_cursors
    }

    pub fn transform_triple(&self) -> &TransformTriple {
        &self.transform_triple
    }

    pub fn buffer(&self) -> &WlBuffer {
        &self.buffer
    }

    pub fn size_buffer(&self) -> &Size<i32, BufferCoord> {
        &self.size_buffer
    }

    /// Send `zwlr_screencopy_frame_v1::ready`.
    ///
    /// Must be called after rendering to the buffer is done.
    pub fn send_ready(mut self) {
        if self.data.is_terminal_state() {
            return;
        }

        self.data
            .transition_state(ScreencopyFrameDataState::Ready)
            .unwrap();

        self.resource
            .flags(zwlr_screencopy_frame_v1::Flags::empty());

        let time = std::time::UNIX_EPOCH.elapsed().unwrap(/* now() is greater than epoch. */);
        let (tv_sec_hi, tv_sec_lo, tv_nsec) = crate::time_util::duration_into_parts(time);
        self.resource.ready(tv_sec_hi, tv_sec_lo, tv_nsec);
    }

    /// Send `zwlr_screencopy_frame_v1::failed`.
    pub fn send_failed(mut self) {
        ScreencopyFrameError::new_no_error_code("")
            .send_error_failed(&self.resource, &mut self.data);
    }
}
