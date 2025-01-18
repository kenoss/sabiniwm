use smithay::utils::{Logical, Size};

pub(crate) trait OutputExt {
    fn current_logical_size(&self) -> Size<i32, Logical>;
}

impl OutputExt for smithay::output::Output {
    fn current_logical_size(&self) -> Size<i32, Logical> {
        let transform = self.current_transform();
        let size = self.current_mode().unwrap().size;
        let scale = self.current_scale().fractional_scale();

        transform
            .transform_size(size)
            .to_f64()
            .to_logical(scale)
            .to_i32_round()
    }
}

pub(crate) trait SizeExt<N, Kind>
where
    N: smithay::utils::Coordinate,
{
    fn to_u32(&self) -> Result<Size<u32, Kind>, <u32 as TryFrom<N>>::Error>
    where
        u32: TryFrom<N>;
}

impl<N, Kind> SizeExt<N, Kind> for smithay::utils::Size<N, Kind>
where
    N: smithay::utils::Coordinate,
{
    fn to_u32(&self) -> Result<Size<u32, Kind>, <u32 as TryFrom<N>>::Error>
    where
        u32: TryFrom<N>,
    {
        let w = self.w.try_into()?;
        let h = self.h.try_into()?;
        Ok(Size::from((w, h)))
    }
}
