use crate::zwlr_screencopy_v1::zwlr_screencopy_frame_v1::ScreencopyFrameData;
use crate::zwlr_screencopy_v1::zwlr_screencopy_manager_v1::ScreencopyManagerData;
use crate::zwlr_screencopy_v1::{ScreencopyDispatcher, ScreencopyHandler};
use smithay::reexports::wayland_protocols_wlr::screencopy::v1::server::{
    zwlr_screencopy_frame_v1, zwlr_screencopy_manager_v1,
};
use smithay::reexports::wayland_server::{
    Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New,
};

const ZWLR_SCREENCOPY_MANAGER_V1_VERSION: u32 = 3;

/// Crate internal data. Public only for `delegate_zwlr_screencopy_v1!`.
#[doc(hidden)]
pub struct ScreencopyGlobalData {
    filter: Box<dyn for<'a> Fn(&'a Client) -> bool + Send + Sync>,
}

/// Creates global data for `zwlr_screencopy_manager_v1`.
///
/// `GlobalData` of zwlr_screencopy_manager_v1
pub struct ScreencopyState;

impl ScreencopyState {
    pub fn new<D, F>(display: &DisplayHandle, filter: F) -> Self
    where
        D: GlobalDispatch<
                zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1,
                ScreencopyGlobalData,
            > + Dispatch<zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1, ScreencopyManagerData>
            + Dispatch<zwlr_screencopy_frame_v1::ZwlrScreencopyFrameV1, ScreencopyFrameData>
            + ScreencopyHandler
            + 'static,
        F: for<'a> Fn(&'a Client) -> bool + Send + Sync + 'static,
    {
        let global_data = ScreencopyGlobalData {
            filter: Box::new(filter),
        };
        display.create_global::<D, zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1, _>(
            ZWLR_SCREENCOPY_MANAGER_V1_VERSION,
            global_data,
        );

        Self
    }
}

impl<D> GlobalDispatch<zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1, ScreencopyGlobalData, D>
    for ScreencopyDispatcher
where
    D: GlobalDispatch<zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1, ScreencopyGlobalData>
        + Dispatch<zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1, ScreencopyManagerData>
        + Dispatch<zwlr_screencopy_frame_v1::ZwlrScreencopyFrameV1, ScreencopyFrameData>
        + ScreencopyHandler
        + 'static,
{
    fn bind(
        _state: &mut D,
        _display_handle: &DisplayHandle,
        _client: &Client,
        resource: New<zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1>,
        _global_data: &ScreencopyGlobalData,
        data_init: &mut DataInit<'_, D>,
    ) {
        data_init.init(resource, ScreencopyManagerData);
    }

    fn can_view(client: Client, global_data: &ScreencopyGlobalData) -> bool {
        (global_data.filter)(&client)
    }
}
