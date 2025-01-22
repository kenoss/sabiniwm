use crate::focus::PointerFocusTarget;
use crate::state::SabiniwmState;
use smithay::desktop::{layer_map_for_output, WindowSurfaceType};
use smithay::utils::{Logical, Point};
use smithay::wayland::shell::wlr_layer::Layer as WlrLayer;

impl SabiniwmState {
    pub fn surface_under(
        &self,
        pos: Point<f64, Logical>,
    ) -> Option<(PointerFocusTarget, Point<f64, Logical>)> {
        let output = self.inner.space.outputs().find(|o| {
            let geometry = self.inner.space.output_geometry(o).unwrap();
            geometry.contains(pos.to_i32_round())
        })?;
        let output_geo = self.inner.space.output_geometry(output).unwrap();

        use crate::session_lock::SessionLockState;
        match self.inner.session_lock_data.get_lock_surface(output) {
            SessionLockState::NotLocked => {}
            SessionLockState::Locked(output_assoc) => match &output_assoc.lock_surface {
                Some(lock_surface) => {
                    return Some((
                        PointerFocusTarget::from(lock_surface.wl_surface().clone()),
                        pos.to_i32_round(),
                    ));
                }
                None => {
                    return None;
                }
            },
        }

        let layers = layer_map_for_output(output);

        let mut under = None;
        if let Some(focus) = layers
            .layer_under(WlrLayer::Overlay, pos)
            .or_else(|| layers.layer_under(WlrLayer::Top, pos))
            .and_then(|layer| {
                let layer_loc = layers.layer_geometry(layer).unwrap().loc;
                layer
                    .surface_under(
                        pos - output_geo.loc.to_f64() - layer_loc.to_f64(),
                        WindowSurfaceType::ALL,
                    )
                    .map(|(surface, loc)| {
                        (
                            PointerFocusTarget::from(surface),
                            loc + layer_loc + output_geo.loc,
                        )
                    })
            })
        {
            under = Some(focus)
        } else if let Some(focus) = self
            .inner
            .space
            .element_under(pos)
            .and_then(|(window, loc)| {
                window
                    .surface_under(pos - loc.to_f64(), WindowSurfaceType::ALL)
                    .map(|(surface, surf_loc)| (surface.into(), surf_loc + loc))
            })
        {
            under = Some(focus)
        } else if let Some(focus) = layers
            .layer_under(WlrLayer::Bottom, pos)
            .or_else(|| layers.layer_under(WlrLayer::Background, pos))
            .and_then(|layer| {
                let layer_loc = layers.layer_geometry(layer).unwrap().loc;
                layer
                    .surface_under(
                        pos - output_geo.loc.to_f64() - layer_loc.to_f64(),
                        WindowSurfaceType::ALL,
                    )
                    .map(|(surface, loc)| {
                        (
                            PointerFocusTarget::from(surface),
                            loc + layer_loc + output_geo.loc,
                        )
                    })
            })
        {
            under = Some(focus)
        };
        under.map(|(focus_target, loc)| (focus_target, loc.to_f64()))
    }
}
