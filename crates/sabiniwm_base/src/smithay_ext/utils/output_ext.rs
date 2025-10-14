use smithay::utils::{Logical, Size};

pub trait OutputExt {
    // Panics: Panics if `Output::current_mode()` is `None`.
    fn current_logical_size(&self) -> Size<i32, Logical>;
}

impl OutputExt for smithay::output::Output {
    fn current_logical_size(&self) -> Size<i32, Logical> {
        let transform = self.current_transform();
        let size = self
            .current_mode()
            .expect("Output::current_mode() is set")
            .size;
        let scale = self.current_scale().fractional_scale();

        transform
            .transform_size(size)
            .to_f64()
            .to_logical(scale)
            .to_i32_round()
    }
}
