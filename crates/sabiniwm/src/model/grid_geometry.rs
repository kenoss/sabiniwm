use crate::view::window::Thickness;
use smithay::utils::{Logical, Point, Rectangle, Size};
use std::ops::Range;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitSpec {
    FixedSize(usize),
    Elastic,
}

#[allow(dead_code)]
pub trait RectangleExt: Sized {
    fn from_ranges(xr: Range<i32>, yr: Range<i32>) -> Self;
    fn split_vertically_2(&self, specs: [SplitSpec; 2]) -> [Self; 2];
    fn split_horizontally_2(&self, specs: [SplitSpec; 2]) -> [Self; 2];
    fn split_vertically(&self, specs: &[SplitSpec]) -> Vec<Self>;
    fn split_horizontally(&self, specs: &[SplitSpec]) -> Vec<Self>;
    fn shrink(&self, dim: Thickness) -> Self;
    fn inflate(&self, dim: Thickness) -> Self;
}

impl RectangleExt for Rectangle<i32, Logical> {
    fn from_ranges(xr: Range<i32>, yr: Range<i32>) -> Rectangle<i32, Logical> {
        let loc = Point::from((xr.start, yr.start));
        let size = Size::from((xr.end - xr.start, yr.end - yr.start));
        Rectangle::new(loc, size)
    }

    fn split_vertically_2(&self, specs: [SplitSpec; 2]) -> [Rectangle<i32, Logical>; 2] {
        let xr = self.loc.x..(self.loc.x + self.size.w);
        let yr = self.loc.y..(self.loc.y + self.size.h);
        let [r0, r1] = split_range_2(specs, &xr);
        [
            Rectangle::from_ranges(r0, yr.clone()),
            Rectangle::from_ranges(r1, yr),
        ]
    }

    fn split_horizontally_2(&self, specs: [SplitSpec; 2]) -> [Rectangle<i32, Logical>; 2] {
        let xr = self.loc.x..(self.loc.x + self.size.w);
        let yr = self.loc.y..(self.loc.y + self.size.h);
        let [r0, r1] = split_range_2(specs, &yr);
        [
            Rectangle::from_ranges(xr.clone(), r0),
            Rectangle::from_ranges(xr, r1),
        ]
    }

    fn split_vertically(&self, specs: &[SplitSpec]) -> Vec<Rectangle<i32, Logical>> {
        let xr = self.loc.x..(self.loc.x + self.size.w);
        let yr = self.loc.y..(self.loc.y + self.size.h);
        split_range(specs, &xr)
            .into_iter()
            .map(|r| Rectangle::from_ranges(r, yr.clone()))
            .collect()
    }

    fn split_horizontally(&self, specs: &[SplitSpec]) -> Vec<Rectangle<i32, Logical>> {
        let xr = self.loc.x..(self.loc.x + self.size.w);
        let yr = self.loc.y..(self.loc.y + self.size.h);
        split_range(specs, &yr)
            .into_iter()
            .map(|r| Rectangle::from_ranges(xr.clone(), r))
            .collect()
    }

    fn shrink(&self, dim: Thickness) -> Rectangle<i32, Logical> {
        let Thickness {
            top,
            right,
            bottom,
            left,
        } = dim;
        let (top, right, bottom, left) = (top as i32, right as i32, bottom as i32, left as i32);
        let loc = Point::from((self.loc.x + right, self.loc.y + top));
        let w = right + left;
        let h = top + bottom;
        let size = Size::from((0.max(self.size.w - w), 0.max(self.size.h - h)));
        Rectangle::new(loc, size)
    }

    fn inflate(&self, dim: Thickness) -> Rectangle<i32, Logical> {
        let Thickness {
            top,
            right,
            bottom,
            left,
        } = dim;
        let (top, right, bottom, left) = (top as i32, right as i32, bottom as i32, left as i32);
        let loc = Point::from((self.loc.x - right, self.loc.y - top));
        let w = right + left;
        let h = top + bottom;
        let size = Size::from((self.size.w + w, self.size.h + h));
        Rectangle::new(loc, size)
    }
}

fn split_range_2(specs: [SplitSpec; 2], r: &Range<i32>) -> [Range<i32>; 2] {
    use SplitSpec::*;

    let w = r.end - r.start;
    let mid = match specs {
        [FixedSize(n), FixedSize(m)] => {
            let n = n as i32;
            let m = m as i32;
            assert_eq!(n + m, w);
            r.start + n
        }
        [FixedSize(n), Elastic] => {
            let n = n as i32;
            assert!(n <= w);
            r.start + n
        }
        [Elastic, FixedSize(n)] => {
            let n = n as i32;
            assert!(n <= w);
            r.end - n
        }
        [Elastic, Elastic] => r.start + w / 2,
    };
    [r.start..mid, mid..r.end]
}

fn split_range(specs: &[SplitSpec], r: &Range<i32>) -> Vec<Range<i32>> {
    use SplitSpec::*;

    let w = r.end - r.start;
    let fixed_size_sum: usize = specs
        .iter()
        .map(|s| match s {
            FixedSize(n) => *n,
            Elastic => 0,
        })
        .sum();
    let fixed_size_sum = fixed_size_sum as i32;
    let elastic_count = specs.iter().filter(|&&s| s == Elastic).count() as i32;
    assert!(fixed_size_sum <= w);
    let elastic_size_sum = w - fixed_size_sum;
    let (elastic_size_base, mut rest) = if elastic_count == 0 {
        assert!(elastic_size_sum == 0);
        (0, 0)
    } else {
        (
            elastic_size_sum / elastic_count,
            elastic_size_sum % elastic_count,
        )
    };
    let mut i = r.start;
    let mut rs = vec![];
    for spec in specs {
        let n = match spec {
            FixedSize(n) => *n as i32,
            Elastic => {
                if rest == 0 {
                    elastic_size_base
                } else {
                    rest -= 1;
                    elastic_size_base + 1
                }
            }
        };
        rs.push(i..i + n);
        i += n;
    }
    rs
}
