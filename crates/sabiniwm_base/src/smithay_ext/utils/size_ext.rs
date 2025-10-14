use smithay::utils::{Coordinate, Size};

pub trait SizeExt<N, Kind>
where
    N: Coordinate,
{
    fn to_u32(&self) -> Result<Size<u32, Kind>, <u32 as TryFrom<N>>::Error>
    where
        u32: TryFrom<N>;
}

impl<N, Kind> SizeExt<N, Kind> for Size<N, Kind>
where
    N: Coordinate,
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
