use super::{FixedTransform, RectangleExt};
use smithay::utils::{Coordinate, Logical, Physical, Point, Rectangle, Size};

#[derive(Debug, Clone, Copy)]
pub enum CoordSystem {
    // Origin bottom-left
    Math,
    // Origin top-left
    Screen,
}

/// Data of affine tarnsformation, which maps things in logical coordinates to physical coordinates.
///
/// ```text
///                Scale        Transform             Size<i32, Physical>
///                 |            |                     |
///   point_logical -> point_mid -> point_physical in output
/// ```
///
/// Note that transform can be applyied before scaling, but we fix the order to avoid confusion.
#[derive(Debug)]
pub struct TransformTriple {
    /// Size of output
    pub size: Size<i32, Physical>,
    /// Scale converting things in logical coordinates to mid
    pub scale: smithay::output::Scale,
    /// Transform converting things in mid to physical coordinates of output
    pub transform: FixedTransform,
}

// Note that we don't impl `Eq` as it contains comparison of `f64`.
impl PartialEq for TransformTriple {
    fn eq(&self, other: &Self) -> bool {
        fn scale_eq(lhs: &smithay::output::Scale, rhs: &smithay::output::Scale) -> bool {
            use smithay::output::Scale;

            match (lhs, rhs) {
                (Scale::Integer(lhs), Scale::Integer(rhs)) => lhs == rhs,
                (Scale::Fractional(lhs), Scale::Fractional(rhs)) => lhs == rhs,
                (
                    Scale::Custom {
                        advertised_integer: advertised_integer_lhs,
                        fractional: fractional_lhs,
                    },
                    Scale::Custom {
                        advertised_integer: advertised_integer_rhs,
                        fractional: fractional_rhs,
                    },
                ) => {
                    advertised_integer_lhs == advertised_integer_rhs
                        && fractional_lhs == fractional_rhs
                }
                (_, _) => false,
            }
        }

        self.size == other.size
            && scale_eq(&self.scale, &other.scale)
            && self.transform == other.transform
    }
}

#[derive(thiserror::Error, Debug)]
pub enum TransformTripleNewError {
    #[error("Output::current_mode() must be set")]
    OutputCurrentModeNotSet,
}

impl TransformTriple {
    // See #winit-flipped180 in note/issue-transform.md.
    //
    // Create a corrected transform by t_a.s = t_t^{-1}.t_t.t_a.s.
    //
    // Note that we need to handle #Transform-invert-wrong to get a proper t_t^{-1}.
    pub fn new_with_terminal_correction(
        output: &smithay::output::Output,
        coord_system: CoordSystem,
    ) -> Result<Self, TransformTripleNewError> {
        let terminal_transform = match coord_system {
            CoordSystem::Math => FixedTransform::Flipped180,
            CoordSystem::Screen => FixedTransform::Normal,
        };
        let transform = FixedTransform::from(output.current_transform());
        let transform = terminal_transform.invert().comp(transform);

        let size = output
            .current_mode()
            .ok_or(TransformTripleNewError::OutputCurrentModeNotSet)?
            .size;
        let scale = output.current_scale();
        Ok(TransformTriple {
            size,
            scale,
            transform,
        })
    }

    pub fn new_without_terminal_correction(
        output: &smithay::output::Output,
    ) -> Result<Self, TransformTripleNewError> {
        let transform = FixedTransform::from(output.current_transform());

        let size = output
            .current_mode()
            .ok_or(TransformTripleNewError::OutputCurrentModeNotSet)?
            .size;
        let scale = output.current_scale();
        Ok(TransformTriple {
            size,
            scale,
            transform,
        })
    }

    pub fn size_mid(&self) -> Size<i32, Physical> {
        self.transform.invert().transform_size(&self.size)
    }

    pub fn size_logical(&self) -> Size<i32, Logical> {
        let scale = self.scale.fractional_scale();
        self.size_mid().to_f64().to_logical(scale).to_i32_round()
    }

    fn map_point_mid_to_physical<N>(&self, p: &Point<N, Physical>) -> Point<N, Physical>
    where
        N: Coordinate + From<i32>,
    {
        #![allow(clippy::clone_on_copy)]

        let size_mid = self.size_mid();
        let s: Size<N, Physical> = (size_mid.w.into(), size_mid.h.into()).into();
        match self.transform {
            FixedTransform::Normal => p.clone(),
            FixedTransform::_90 => (p.y, s.w - p.x).into(),
            FixedTransform::_180 => (s.w - p.x, s.h - p.y).into(),
            FixedTransform::_270 => (s.h - p.y, p.x).into(),
            FixedTransform::Flipped => (s.w - p.x, p.y).into(),
            FixedTransform::Flipped90 => (p.y, p.x).into(),
            FixedTransform::Flipped180 => (p.x, s.h - p.y).into(),
            FixedTransform::Flipped270 => (s.h - p.y, s.w - p.x).into(),
        }
    }

    // Map a logical rect to a physical rect before transform.
    pub fn map_rect_logical_to_mid(
        &self,
        rect: &Rectangle<i32, Logical>,
    ) -> Rectangle<i32, Physical> {
        let scale = self.scale.fractional_scale();
        rect.to_physical_precise_round(scale)
    }

    fn map_rect_mid_to_physical<N>(&self, rect: &Rectangle<N, Physical>) -> Rectangle<N, Physical>
    where
        N: Coordinate + From<i32>,
    {
        // Choose a corner that is mapped to left-top in codomain.
        let point_mid_mapped_left_top = match self.transform {
            FixedTransform::Normal => rect.left_top(),
            FixedTransform::_90 => rect.right_top(),
            FixedTransform::_180 => rect.right_bottom(),
            FixedTransform::_270 => rect.left_bottom(),
            FixedTransform::Flipped => rect.right_top(),
            FixedTransform::Flipped90 => rect.left_top(),
            FixedTransform::Flipped180 => rect.left_bottom(),
            FixedTransform::Flipped270 => rect.right_bottom(),
        };
        let left_top_physical = self.map_point_mid_to_physical(&point_mid_mapped_left_top);
        let size_physical = self.transform.transform_size(&rect.size);
        Rectangle::new(left_top_physical, size_physical)
    }

    // Map a logical rect to a physical rect.
    pub fn map_rect_logical_to_physical(
        &self,
        rect: &Rectangle<i32, Logical>,
    ) -> Rectangle<i32, Physical> {
        let scale = self.scale.fractional_scale();
        let rect_mid = rect.to_physical_precise_round(scale);
        self.map_rect_mid_to_physical(&rect_mid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smithay_ext::utils::RectangleExt;
    use rstest::rstest;
    use smithay::utils::Logical;

    #[rstest(
        // #[rustfmt::skip]
        transform,
        scale,
        size_physical,
        size_logical,
        rect_logical,
        rect_physical,
        n_shared_corners,

        // Check the behavior of entire rect, `size_logical == rect_logical.size`.
        case(FixedTransform::Normal, 1.6, (1280, 960), (800, 600), ((0, 0), (800, 600)), ((0, 0), (1280, 960)), 4),
        case(FixedTransform::_90,    1.6, (1280, 960), (600, 800), ((0, 0), (600, 800)), ((0, 0), (1280, 960)), 4),

        // Check the behavior of small rect at left-top corner in logical.
        //
        // Notice that
        //
        //   80   = 800 * 0.1
        //   60   = 600 * 0.1
        //   128  = 80 * 1.6
        //   96   = 60 * 1.6
        //   1152 = 1280 - 128
        //   864  = 960 - 96
        //
        // Only scaling
        case(FixedTransform::Normal,     1.6, (1280, 960), (800, 600), ((0, 0), (80, 60)), ((0, 0),      (128, 96)), 1),
        // Mapped to left-bottom
        case(FixedTransform::_90,        1.6, (1280, 960), (600, 800), ((0, 0), (60, 80)), ((0, 864),    (128, 96)), 1),
        // Mapped to right-bottom
        case(FixedTransform::_180,       1.6, (1280, 960), (800, 600), ((0, 0), (80, 60)), ((1152, 864), (128, 96)), 1),
        // Mapped to right-top
        case(FixedTransform::_270,       1.6, (1280, 960), (600, 800), ((0, 0), (60, 80)), ((1152, 0),   (128, 96)), 1),
        // Mapped to right-top
        case(FixedTransform::Flipped,    1.6, (1280, 960), (800, 600), ((0, 0), (80, 60)), ((1152, 0),   (128, 96)), 1),
        // Mapped to left-top
        case(FixedTransform::Flipped90,  1.6, (1280, 960), (600, 800), ((0, 0), (60, 80)), ((0, 0),      (128, 96)), 1),
        // Mapped to left-bottom
        case(FixedTransform::Flipped180, 1.6, (1280, 960), (800, 600), ((0, 0), (80, 60)), ((0, 864),    (128, 96)), 1),
        // Mapped to right-bottom
        case(FixedTransform::Flipped270, 1.6, (1280, 960), (600, 800), ((0, 0), (60, 80)), ((1152, 864), (128, 96)), 1),

        // Check the behavior of small rect near left-top corner in logical.
        //
        // Only scaling
        case(FixedTransform::Normal,     1.6, (1280, 960), (800, 600), ((80, 60), (80, 60)), ((128, 96),   (128, 96)), 0),
        // Mapped to left-bottom
        case(FixedTransform::_90,        1.6, (1280, 960), (600, 800), ((60, 80), (60, 80)), ((128, 768),  (128, 96)), 0),
        // Mapped to right-bottom
        case(FixedTransform::_180,       1.6, (1280, 960), (800, 600), ((80, 60), (80, 60)), ((1024, 768), (128, 96)), 0),
        // Mapped to right-top
        case(FixedTransform::_270,       1.6, (1280, 960), (600, 800), ((60, 80), (60, 80)), ((1024, 96),  (128, 96)), 0),
        // Mapped to right-top
        case(FixedTransform::Flipped,    1.6, (1280, 960), (800, 600), ((80, 60), (80, 60)), ((1024, 96),  (128, 96)), 0),
        // Mapped to left-top
        case(FixedTransform::Flipped90,  1.6, (1280, 960), (600, 800), ((60, 80), (60, 80)), ((128, 96),   (128, 96)), 0),
        // Mapped to left-bottom
        case(FixedTransform::Flipped180, 1.6, (1280, 960), (800, 600), ((80, 60), (80, 60)), ((128, 768),  (128, 96)), 0),
        // Mapped to right-bottom
        case(FixedTransform::Flipped270, 1.6, (1280, 960), (600, 800), ((60, 80), (60, 80)), ((1024, 768), (128, 96)), 0),

        ::trace
    )]
    fn test_map_rect_logical_to_physical(
        size_physical: impl Into<Size<i32, Physical>>,
        transform: FixedTransform,
        scale: f64,
        size_logical: impl Into<Size<i32, Logical>>,
        rect_logical: (
            impl Into<Point<i32, Logical>>,
            impl Into<Size<i32, Logical>>,
        ),
        rect_physical: (
            impl Into<Point<i32, Physical>>,
            impl Into<Size<i32, Physical>>,
        ),
        n_shared_corners: usize,
    ) {
        let size_physical = size_physical.into();
        let size_logical = size_logical.into();
        let scale = smithay::output::Scale::Custom {
            advertised_integer: 1,
            fractional: scale,
        };
        let tt = TransformTriple {
            size: size_physical,
            scale,
            transform,
        };
        assert_eq!(tt.size_logical(), size_logical);

        let rect_logical: Rectangle<i32, Logical> =
            Rectangle::new(rect_logical.0.into(), rect_logical.1.into());
        let rect_physical: Rectangle<i32, Physical> =
            Rectangle::new(rect_physical.0.into(), rect_physical.1.into());

        let got = tt.map_rect_logical_to_physical(rect_logical);
        assert_eq!(got, rect_physical);

        // Check the number of shared corners among the corners of output and ones of rect.
        let output_rect = Rectangle::new((0, 0).into(), size_physical);
        let corners_of_output = [
            output_rect.left_top(),
            output_rect.left_bottom(),
            output_rect.right_top(),
            output_rect.right_bottom(),
        ];
        let corners_of_rect = [
            got.left_top(),
            got.left_bottom(),
            got.right_top(),
            got.right_bottom(),
        ];
        let mut i = 0;
        for x in &corners_of_output {
            for y in &corners_of_rect {
                if x == y {
                    i += 1;
                }
            }
        }
        assert_eq!(i, n_shared_corners);
    }

    /// #Transform-transform_rect_in-wrong
    ///
    /// See note/issue-transform.md.
    // Failing test for bug. Remove the cfg to run it.
    #[cfg(feature = "failing_test_for_bug")]
    #[rstest(
        // #[rustfmt::skip]
        transform,
        scale,
        size_physical,
        size_logical,
        rect_logical,
        rect_physical,
        _n_shared_corners,

        // It should map the rect to left-bottom, but it does as `Transform::_270`: ((1152, 0), (128, 96))
        case(smithay::utils::Transform::_90, 1.6, (1280, 960), (600, 800), ((0, 0), (60, 80)), ((0, 864), (128, 96)), 1),
        // It should map the rect to left-top, but it does as `Transform::Flipped270`: ((1152, 864), (128, 96))
        case(smithay::utils::Transform::Flipped90, 1.6, (1280, 960), (600, 800), ((0, 0), (60, 80)), ((0, 0), (128, 96)), 1),

        ::trace
    )]
    fn smithay_bug_transform_transform_rect_in(
        transform: smithay::utils::Transform,
        scale: f64,
        size_physical: impl Into<Size<i32, Physical>>,
        size_logical: impl Into<Size<i32, Logical>>,
        rect_logical: (
            impl Into<Point<i32, Logical>>,
            impl Into<Size<i32, Logical>>,
        ),
        rect_physical: (
            impl Into<Point<i32, Physical>>,
            impl Into<Size<i32, Physical>>,
        ),
        _n_shared_corners: usize,
    ) {
        let size_physical = size_physical.into();
        let scale = smithay::output::Scale::Custom {
            advertised_integer: 1,
            fractional: scale,
        };
        let rect_logical: Rectangle<i32, Logical> =
            Rectangle::new(rect_logical.0.into(), rect_logical.1.into());
        let rect_physical: Rectangle<i32, Physical> =
            Rectangle::new(rect_physical.0.into(), rect_physical.1.into());

        let rect_mid = rect_logical
            .to_f64()
            .to_physical(scale.fractional_scale())
            .to_i32_round();
        let size_mid = transform.invert().transform_size(size_physical);
        {
            // Variable names as in `Transform::transform_rect_in()`.
            let area = size_mid;
            let rect = rect_mid;
            dbg!(&rect);
            dbg!(&area);
            dbg!((area.h, rect.loc.y, rect.size.h, rect.loc.x));
            dbg!((area.h - rect.loc.y - rect.size.h, rect.loc.x));
            let got = transform.transform_rect_in(rect, &area);
            assert_eq!(got, rect_physical);
        }
    }
}
