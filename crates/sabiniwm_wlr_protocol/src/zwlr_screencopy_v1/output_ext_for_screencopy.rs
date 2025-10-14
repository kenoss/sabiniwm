use crate::zwlr_screencopy_v1::zwlr_screencopy_frame_v1::ScreencopyFrameRequest;
use itertools::Itertools;
use sabiniwm_base::smithay_ext::utils::TransformTriple;
use std::sync::Mutex;

struct ScreencopyFrameRequests(Mutex<Vec<ScreencopyFrameRequest>>);

/// Helper trait to [`smithay::output::Output`] hold [`ScreencopyFrameRequest`].
pub trait OutputExtForScreencopy {
    /// Adds `ScreenFrameRequest` to `Output` to wait for the next render.
    fn add_screencopy_frame_request(&mut self, frame_request: ScreencopyFrameRequest);
    /// Takes `ScreenFrameRequest`s to render.
    fn take_screencopy_frame_requests(&mut self) -> Vec<ScreencopyFrameRequest>;
}

impl OutputExtForScreencopy for smithay::output::Output {
    fn add_screencopy_frame_request(&mut self, frame_request: ScreencopyFrameRequest) {
        self.user_data()
            .insert_if_missing_threadsafe(|| ScreencopyFrameRequests(Mutex::new(vec![])));
        self.user_data()
            .get::<ScreencopyFrameRequests>()
            .unwrap()
            .0
            .lock()
            .unwrap()
            .push(frame_request);
    }

    fn take_screencopy_frame_requests(&mut self) -> Vec<ScreencopyFrameRequest> {
        let tt = TransformTriple::new_without_terminal_correction(self)
            .unwrap(/* Output::current_mode() is set */);
        self.user_data()
            .get::<ScreencopyFrameRequests>()
            .map(|requests| {
                // If output size is changed, invalidate outdated frame requests.
                let mut requests_by_is_valid = requests
                    .0
                    .lock()
                    .unwrap()
                    .drain(..)
                    .into_group_map_by(|request| *request.transform_triple() == tt);
                if let Some(requests) = requests_by_is_valid.remove(&false) {
                    info!("invalidate outdated frame requests due to update of output, requests.len() = {}",
                          requests.len());
                    for request in requests {
                        request.send_failed();
                    }
                }
                requests_by_is_valid.remove(&true).unwrap_or_default()
            })
            .unwrap_or_default()
    }
}
