use crate::action::action::ActionFnI;
use crate::state::SabiniwmState;
use sabiniwm_base::smithay_ext::utils::FixedTransform;

#[derive(Debug, Clone)]
pub struct ActionOutputApplyTransform(pub FixedTransform);

impl ActionFnI for ActionOutputApplyTransform {
    fn exec(&self, state: &mut SabiniwmState) {
        use smithay::utils::Transform;

        // Limitation: This code works well only for single output.
        //
        // TODO: Support multiple outputs.
        let output = state.inner.space.outputs().next();
        if let Some(output) = output {
            let t = output.current_transform();
            let t = FixedTransform::from(t);
            // Note that `self.0.comp(t)` is more natural, as it "adds" given transform to the
            // current one. But we don't do that because we (and also other Anvil's children) use
            // `Transform::Flipped180` for terminal transform, i.e. for winit backend to y-invert
            // for GLES math cordinates.
            //
            // For more details, see note/issue-transform.md.
            //
            // TODO: Fix it if we fixed `Transform::Flipped180` for winit backend.
            let t = t.comp(self.0);
            let t = Transform::from(t);
            output.change_current_state(None, Some(t), None, None);

            let size = state.inner.space.output_geometry(output)
                .unwrap(/* Space::map_output() and Output::change_current_state() is called. */)
                .size;
            state.inner.view.resize_output(size, &mut state.inner.space);
        }
    }
}
