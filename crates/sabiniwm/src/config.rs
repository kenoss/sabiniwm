use smithay::reexports::drm;

/// Unstable configuration points.
///
/// Do not expect it is stable over major change of semver.
///
/// Please start discussion on GitHub if you have an opinion, e.g. adding/changing configuration points.
#[thin_delegate::register]
pub trait ConfigDelegateUnstableI {
    fn select_mode_and_scale_on_connecter_added(
        &self,
        connector_info: &drm::control::connector::Info,
    ) -> (drm::control::Mode, smithay::output::Scale) {
        unstable_default::select_mode_and_scale_on_connecter_added(connector_info)
    }
}

#[thin_delegate::register]
pub(crate) struct ConfigDelegate {
    inner: Box<dyn ConfigDelegateUnstableI>,
}

#[thin_delegate::derive_delegate(
    scheme = |f| {
        use std::ops::Deref;

        f(self.inner.deref())
    }
)]
impl ConfigDelegateUnstableI for ConfigDelegate {}

impl ConfigDelegate {
    pub fn new(inner: Box<dyn ConfigDelegateUnstableI>) -> Self {
        Self { inner }
    }
}

pub struct ConfigDelegateUnstableDefault;

impl ConfigDelegateUnstableI for ConfigDelegateUnstableDefault {}

pub mod unstable_default {
    use super::*;

    pub fn select_mode_and_scale_on_connecter_added(
        connector_info: &drm::control::connector::Info,
    ) -> (drm::control::Mode, smithay::output::Scale) {
        use smithay::reexports::drm::control::ModeTypeFlags;

        for (i, mode) in connector_info.modes().iter().enumerate() {
            let dpi = calc_estimated_dpi(connector_info, mode);
            info!(
                "connector_info.modes()[{}] = {:?}, estimated DPI = {:?}",
                i, mode, dpi
            );
        }

        let mode = *connector_info
            .modes()
            .iter()
            .find(|mode| mode.mode_type().contains(ModeTypeFlags::PREFERRED))
            .unwrap_or(&connector_info.modes()[0]);
        const TARGET_DPI: f64 = 140.0;
        let scale = calc_output_scale(connector_info, &mode, TARGET_DPI);

        info!(
            "selected: mode = {:?}, scale = {:?}, estimated_dpi = {:?}, corrected_dpi = {:?}",
            mode,
            scale,
            calc_estimated_dpi(connector_info, &mode),
            calc_estimated_dpi(connector_info, &mode).map(|x| x / scale.fractional_scale())
        );

        (mode, scale)
    }

    pub fn calc_estimated_dpi(
        connector_info: &drm::control::connector::Info,
        mode: &drm::control::Mode,
    ) -> Option<f64> {
        let phys_size = connector_info.size();
        phys_size.map(|phys_size| {
            let mode_size = mode.size();
            let dpi_w = mode_size.0 as f64 / (phys_size.0 as f64 / 25.4);
            let dpi_h = mode_size.1 as f64 / (phys_size.1 as f64 / 25.4);
            dpi_w.max(dpi_h)
        })
    }

    pub fn calc_output_scale(
        connector_info: &drm::control::connector::Info,
        mode: &drm::control::Mode,
        target_dpi: f64,
    ) -> smithay::output::Scale {
        let Some(dpi) = calc_estimated_dpi(connector_info, mode) else {
            return smithay::output::Scale::Integer(1);
        };

        // If it's not HiDPI display, don't use scale.
        if dpi <= target_dpi {
            return smithay::output::Scale::Integer(1);
        }

        let logical_from_actual = target_dpi / dpi;
        // Find an approximate value that makes the logical output size an integer.
        // Note that many display sizes are divisible by 8.
        let logical_from_actual = (1.0 / 8.0) * (8.0 * logical_from_actual).round();
        let actual_from_logical = 1.0 / logical_from_actual;
        smithay::output::Scale::Custom {
            advertised_integer: 1,
            fractional: actual_from_logical,
        }
    }
}
