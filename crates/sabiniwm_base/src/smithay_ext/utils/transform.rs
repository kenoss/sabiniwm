use smithay::utils::{Coordinate, Size, Transform};

/// Bug fixed `Transform`
///
/// [wl_output::transform](https://wayland.freedesktop.org/docs/html/apa.html#protocol-spec-wl_output-enum-transform) - transformation applied to buffer contents
///
/// Note that it is a dihedral group `D_4`. See also a comment on `decomp()` in
/// `FixedTransform::comp()`.
///
/// For details of bugs of `Transform`, see note/issue-transform.md.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FixedTransform {
    /// No transform
    #[default]
    Normal,
    /// 90 degrees counter-clockwise
    _90,
    /// 180 degrees counter-clockwise
    _180,
    /// 270 degrees counter-clockwise
    _270,
    /// 180 degree flip around a vertical axis
    Flipped,
    /// Flip and rotate 90 degrees counter-clockwise
    Flipped90,
    /// Flip and rotate 180 degrees counter-clockwise
    Flipped180,
    /// Flip and rotate 270 degrees counter-clockwise
    Flipped270,
}

impl From<Transform> for FixedTransform {
    fn from(x: Transform) -> Self {
        match x {
            Transform::Normal => FixedTransform::Normal,
            Transform::_90 => FixedTransform::_90,
            Transform::_180 => FixedTransform::_180,
            Transform::_270 => FixedTransform::_270,
            Transform::Flipped => FixedTransform::Flipped,
            Transform::Flipped90 => FixedTransform::Flipped90,
            Transform::Flipped180 => FixedTransform::Flipped180,
            Transform::Flipped270 => FixedTransform::Flipped270,
        }
    }
}

impl From<FixedTransform> for Transform {
    fn from(x: FixedTransform) -> Self {
        match x {
            FixedTransform::Normal => Transform::Normal,
            FixedTransform::_90 => Transform::_90,
            FixedTransform::_180 => Transform::_180,
            FixedTransform::_270 => Transform::_270,
            FixedTransform::Flipped => Transform::Flipped,
            FixedTransform::Flipped90 => Transform::Flipped90,
            FixedTransform::Flipped180 => Transform::Flipped180,
            FixedTransform::Flipped270 => Transform::Flipped270,
        }
    }
}

impl FixedTransform {
    pub fn comp(&self, other: FixedTransform) -> FixedTransform {
        fn aux(f: FixedTransform, g: FixedTransform) -> FixedTransform {
            // Decompose the dihedral group `D_4` as a semidirect product `\Z/4\Z \rtimes \Z/2\Z`
            //
            //   f = r^i . s^j,
            //   g = r^i_ . s^j_,
            //
            // where `r = FixedTransform::_90, s = FixedTransform::Flipped`. Then calculate `f.g` by
            // `s . r = r^3 . s`:
            //
            //     f.g
            //   = r^i . s^j . r^i_ . s^j_
            //   = r^i . r^i_ . s^j_                        (if j = 0)
            //   = r^i . r^(3 * i_) . s^(1 + j_)            (if j = 1)
            //   = r^i . r^((2 * j + 1) * i_) . s^(j + j_)

            fn decomp(f: FixedTransform) -> (usize, usize) {
                match f {
                    FixedTransform::Normal => (0, 0),
                    FixedTransform::_90 => (1, 0),
                    FixedTransform::_180 => (2, 0),
                    FixedTransform::_270 => (3, 0),
                    FixedTransform::Flipped => (0, 1),
                    FixedTransform::Flipped90 => (1, 1),
                    FixedTransform::Flipped180 => (2, 1),
                    FixedTransform::Flipped270 => (3, 1),
                }
            }

            let (i, j) = decomp(f);
            let (i_, j_) = decomp(g);
            let i__ = (i + ((2 * j) + 1) * i_).rem_euclid(4);
            let j__ = (j + j_).rem_euclid(2);
            match (i__, j__) {
                (0, 0) => FixedTransform::Normal,
                (1, 0) => FixedTransform::_90,
                (2, 0) => FixedTransform::_180,
                (3, 0) => FixedTransform::_270,
                (0, 1) => FixedTransform::Flipped,
                (1, 1) => FixedTransform::Flipped90,
                (2, 1) => FixedTransform::Flipped180,
                (3, 1) => FixedTransform::Flipped270,
                _ => unreachable!(),
            }
        }

        aux(*self, other)
    }

    // Correct invert implementation of `Transform` (#Transform-invert-wrong)
    //
    // See note/issue-transform.md.
    //
    // TODO: Fix the original implementation.
    pub fn invert(&self) -> FixedTransform {
        match *self {
            FixedTransform::Normal => FixedTransform::Normal,
            FixedTransform::_90 => FixedTransform::_270,
            FixedTransform::_180 => FixedTransform::_180,
            FixedTransform::_270 => FixedTransform::_90,
            FixedTransform::Flipped => FixedTransform::Flipped,
            FixedTransform::Flipped90 => FixedTransform::Flipped90,
            FixedTransform::Flipped180 => FixedTransform::Flipped180,
            FixedTransform::Flipped270 => FixedTransform::Flipped270,
        }
    }

    pub fn transform_size<N, Kind>(&self, size: &Size<N, Kind>) -> Size<N, Kind>
    where
        N: Coordinate,
    {
        #![allow(clippy::clone_on_copy)]

        match *self {
            FixedTransform::Normal
            | FixedTransform::_180
            | FixedTransform::Flipped
            | FixedTransform::Flipped180 => size.clone(),
            FixedTransform::_90
            | FixedTransform::_270
            | FixedTransform::Flipped90
            | FixedTransform::Flipped270 => (size.h, size.w).into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest(
        // #[rustfmt::skip]
        f_g,
        f,
        g,

        case(FixedTransform::Normal,     FixedTransform::Normal,    FixedTransform::Normal),
        case(FixedTransform::Flipped90,  FixedTransform::_90,       FixedTransform::Flipped),
        case(FixedTransform::Flipped180, FixedTransform::_180,      FixedTransform::Flipped),
        case(FixedTransform::Flipped270, FixedTransform::_270,      FixedTransform::Flipped),
        case(FixedTransform::Flipped270, FixedTransform::Flipped,   FixedTransform::_90),
        case(FixedTransform::Flipped180, FixedTransform::Flipped,   FixedTransform::_180),
        case(FixedTransform::Flipped90,  FixedTransform::Flipped,   FixedTransform::_270),
        case(FixedTransform::Flipped,    FixedTransform::Flipped90, FixedTransform::_90),
        case(FixedTransform::Flipped270, FixedTransform::Flipped90, FixedTransform::_180),
        case(FixedTransform::Flipped180, FixedTransform::Flipped90, FixedTransform::_270),

        ::trace
    )]
    fn test_comp(f_g: FixedTransform, f: FixedTransform, g: FixedTransform) {
        assert_eq!(f.comp(g), f_g);
    }

    #[rstest(
        // #[rustfmt::skip]
        f,

        case(FixedTransform::Normal),
        case(FixedTransform::_90),
        case(FixedTransform::_180),
        case(FixedTransform::_270),
        case(FixedTransform::Flipped),
        case(FixedTransform::Flipped90),
        case(FixedTransform::Flipped180),
        case(FixedTransform::Flipped270),

        ::trace
    )]
    fn test_invert_is_left_right_inverse(f: FixedTransform) {
        assert_eq!(f.invert().comp(f), FixedTransform::Normal);
        assert_eq!(f.comp(f.invert()), FixedTransform::Normal);
    }

    /// #Transform-invert-wrong
    ///
    /// See note/issue-transform.md.
    // Failing test for bug. Remove the cfg to run it.
    #[cfg(feature = "failing_test_for_bug")]
    #[rstest(
        // #[rustfmt::skip]
        f,

        case(Transform::Normal),
        case(Transform::_90),
        case(Transform::_180),
        case(Transform::_270),
        case(Transform::Flipped),
        case(Transform::Flipped90),
        case(Transform::Flipped180),
        case(Transform::Flipped270),

        ::trace
    )]
    fn smithay_bug_invert_is_not_left_right_inverse(f: Transform) {
        fn comp(f: Transform, g: Transform) -> Transform {
            let f = FixedTransform::from(f);
            let g = FixedTransform::from(g);
            let f_g = f.comp(g);
            Transform::from(f_g)
        }

        assert_eq!(comp(f.invert(), f), Transform::Normal);
        assert_eq!(comp(f, f.invert()), Transform::Normal);
    }
}
