use crate as sabiniwm;
use smithay::reexports::drm;

/// Unstable configuration points.
///
/// Do not expect it is stable over major change of semver.
///
/// Please start discussion on GitHub if you have an opinion, e.g. adding/changing configuration points.
#[thin_delegate::register]
pub trait ConfigDelegateUnstableI {
    fn get_xkb_config(&self) -> sabiniwm::config::XkbConfig<'_> {
        unstable_default::get_xkb_config()
    }

    fn focus_follows_mouse(&self) -> bool {
        true
    }

    fn make_layout_tree_builder(&self) -> sabiniwm::view::layout_node::LayoutTreeBuilder {
        unstable_default::make_layout_tree_builder()
    }

    fn make_workspace_tags(&self) -> Vec<sabiniwm::view::stackset::WorkspaceTag> {
        use sabiniwm::view::stackset::WorkspaceTag;

        (0..=9).map(|i| WorkspaceTag(format!("{}", i))).collect()
    }

    fn make_keymap(
        &self,
        _is_udev_backend: bool,
    ) -> sabiniwm::input::Keymap<sabiniwm::action::Action> {
        use big_s::S;
        use sabiniwm::action::{self, Action, ActionFnI};
        use sabiniwm::input::{KeySeqSerde, Keymap, ModMask};

        let meta_keys = hashmap! {
            S("C") => ModMask::CONTROL,
            S("M") => ModMask::MOD1,
            S("s") => ModMask::MOD4,
            S("H") => ModMask::MOD5,
        };
        let keyseq_serde = KeySeqSerde::new(meta_keys);
        let kbd = |s| keyseq_serde.kbd(s).unwrap();
        let keymap = hashmap! {
            kbd("C-x C-q") => action::ActionQuitSabiniwm.into_action(),

            kbd("C-x C-t") => Action::spawn("alacritty"),
        };

        Keymap::new(keymap)
    }

    fn select_mode_and_scale_on_connecter_added(
        &self,
        connector_info: &drm::control::connector::Info,
    ) -> (drm::control::Mode, smithay::output::Scale) {
        unstable_default::select_mode_and_scale_on_connecter_added(connector_info)
    }
}

#[derive(Debug)]
pub struct XkbConfig<'a> {
    pub xkb_config: smithay::input::keyboard::XkbConfig<'a>,
    pub repeat_delay: u16,
    pub repeat_rate: u16,
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

    pub fn get_xkb_config() -> sabiniwm::config::XkbConfig<'static> {
        use sabiniwm::config::XkbConfig;

        XkbConfig {
            xkb_config: Default::default(),
            repeat_delay: 200,
            repeat_rate: 60,
        }
    }
    pub fn make_layout_tree_builder() -> sabiniwm::view::layout_node::LayoutTreeBuilder {
        use sabiniwm::util::NonEmptyFocusedVec;
        use sabiniwm::view::layout_node::{LayoutNode, LayoutTreeBuilder};
        use sabiniwm::view::predefined::{
            LayoutFull, LayoutNodeBorder, LayoutNodeMargin, LayoutNodeSelect, LayoutNodeToggle,
            LayoutTall,
        };
        use sabiniwm::view::window::{Border, Rgba};
        use std::collections::HashMap;

        let mut nodes = HashMap::new();

        let node = LayoutNode::from(LayoutTall {});
        let node_id0 = node.id();
        nodes.insert(node_id0, node);

        let node = LayoutNode::from(LayoutFull {});
        let node_id1 = node.id();
        nodes.insert(node_id1, node);

        let layouts = NonEmptyFocusedVec::new(vec![node_id0, node_id1], 0);
        let node = LayoutNode::from(LayoutNodeSelect::new(layouts));
        let node_id = node.id();
        nodes.insert(node_id, node);

        let margin = 8.into();
        let node = LayoutNode::from(LayoutNodeMargin::new(node_id, margin));
        let node_id = node.id();
        nodes.insert(node_id, node);

        let border = Border {
            dim: 2.into(),
            active_rgba: Rgba::from_rgba(0x556b2fff),
            inactive_rgba: Rgba::from_rgba(0x00000000),
        };
        let node = LayoutNode::from(LayoutNodeBorder::new(node_id, border));
        let node_id = node.id();
        nodes.insert(node_id, node);

        let node = LayoutNode::from(LayoutFull {});
        let node_id_full = node.id();
        nodes.insert(node_id_full, node);

        let node = LayoutNode::from(LayoutNodeToggle::new(node_id, node_id_full));
        let node_id = node.id();
        nodes.insert(node_id, node);

        LayoutTreeBuilder::new(nodes, node_id)
    }

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
