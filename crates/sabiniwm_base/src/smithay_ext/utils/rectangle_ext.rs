use smithay::utils::{Coordinate, Point, Rectangle};

pub trait RectangleExt<N, Kind>
where
    N: Coordinate,
{
    fn left_top(&self) -> Point<N, Kind>;
    fn left_bottom(&self) -> Point<N, Kind>;
    fn right_top(&self) -> Point<N, Kind>;
    fn right_bottom(&self) -> Point<N, Kind>;
}

impl<N, Kind> RectangleExt<N, Kind> for Rectangle<N, Kind>
where
    N: Coordinate,
{
    fn left_top(&self) -> Point<N, Kind> {
        self.loc
    }

    fn left_bottom(&self) -> Point<N, Kind> {
        (self.loc.x, self.loc.y + self.size.h).into()
    }

    fn right_top(&self) -> Point<N, Kind> {
        (self.loc.x + self.size.w, self.loc.y).into()
    }

    fn right_bottom(&self) -> Point<N, Kind> {
        (self.loc.x + self.size.w, self.loc.y + self.size.h).into()
    }
}
