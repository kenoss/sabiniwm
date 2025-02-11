// focus == 0 if vec.is_empty()
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusedVec<T> {
    vec: Vec<T>,
    focus: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NonEmptyFocusedVec<T> {
    vec: Vec<T>,
    focus: usize,
}

#[derive(Debug)]
pub struct FocusedVecUpdateGuard<'a, T> {
    p: *mut FocusedVec<T>,
    pub vec: &'a mut Vec<T>,
    // Use immediate value instead of `&mut usize` for ergonomics.
    pub focus: usize,
}

#[derive(Debug)]
pub struct NonEmptyFocusedVecUpdateGuard<'a, T> {
    p: *mut NonEmptyFocusedVec<T>,
    pub vec: &'a mut Vec<T>,
    // Use immediate value instead of `&mut usize` for ergonomics.
    pub focus: usize,
}

impl<T> Default for FocusedVec<T> {
    fn default() -> Self {
        Self {
            vec: vec![],
            focus: 0,
        }
    }
}

impl<T> FocusedVec<T> {
    pub fn new(vec: Vec<T>, focus: usize) -> Self {
        let this = Self { vec, focus };
        this.assert_invariant();
        this
    }

    #[inline]
    fn assert_invariant(&self) {
        assert!(self.focus < self.vec.len() || self.vec.is_empty());
    }

    #[inline]
    pub fn as_mut(&mut self) -> FocusedVecUpdateGuard<'_, T> {
        FocusedVecUpdateGuard {
            p: self as *mut FocusedVec<T>,
            vec: &mut self.vec,
            focus: self.focus,
        }
    }

    #[inline]
    pub fn as_vec(&self) -> &Vec<T> {
        &self.vec
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.vec.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline]
    pub fn push(&mut self, x: T) {
        self.vec.push(x);
    }

    #[inline]
    pub fn focus(&self) -> Option<&T> {
        self.vec.get(self.focus)
    }

    #[inline]
    pub fn focus_mut(&mut self) -> Option<&mut T> {
        self.vec.get_mut(self.focus)
    }

    #[inline]
    pub fn focused_index(&self) -> usize {
        self.focus
    }

    #[inline]
    pub fn set_focused_index(&mut self, i: usize) {
        self.focus = i;

        self.assert_invariant();
    }

    pub fn mod_plus_focused_index(&self, diff: isize) -> usize {
        if self.is_empty() {
            return 0;
        }

        mod_plus_focused_index(self.vec.len(), self.focus, diff)
    }
}

impl<T> NonEmptyFocusedVec<T> {
    #[inline]
    pub fn new(vec: Vec<T>, focus: usize) -> Self {
        let this = Self { vec, focus };
        this.assert_invariant();
        this
    }

    #[inline]
    fn assert_invariant(&self) {
        assert!(self.focus < self.vec.len());
        assert!(!self.vec.is_empty());
    }

    #[inline]
    pub fn as_mut(&mut self) -> NonEmptyFocusedVecUpdateGuard<'_, T> {
        NonEmptyFocusedVecUpdateGuard {
            p: self as *mut NonEmptyFocusedVec<T>,
            vec: &mut self.vec,
            focus: self.focus,
        }
    }

    #[inline]
    pub fn as_vec(&self) -> &Vec<T> {
        &self.vec
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.vec.len()
    }

    #[inline]
    pub fn push(&mut self, x: T) {
        self.vec.push(x);
    }

    #[inline]
    pub fn focus(&self) -> &T {
        &self.vec[self.focus]
    }

    #[inline]
    pub fn focus_mut(&mut self) -> &mut T {
        &mut self.vec[self.focus]
    }

    #[inline]
    pub fn focused_index(&self) -> usize {
        self.focus
    }

    #[inline]
    pub fn set_focused_index(&mut self, i: usize) {
        self.focus = i;

        self.assert_invariant();
    }

    pub fn mod_plus_focused_index(&self, diff: isize) -> usize {
        mod_plus_focused_index(self.vec.len(), self.focus, diff)
    }
}

impl<T> Drop for FocusedVecUpdateGuard<'_, T> {
    fn drop(&mut self) {
        // Safety: Having ownership of `vec`.
        let p: &mut FocusedVec<T> = unsafe { &mut *self.p };
        p.focus = self.focus;

        p.assert_invariant();
    }
}

impl<T> FocusedVecUpdateGuard<'_, T> {
    pub fn commit(self) {
        drop(self);
    }

    pub fn mod_plus_focused_index(&self, diff: isize) -> usize {
        if self.vec.is_empty() {
            return 0;
        }

        mod_plus_focused_index(self.vec.len(), self.focus, diff)
    }
}

impl<T> Drop for NonEmptyFocusedVecUpdateGuard<'_, T> {
    fn drop(&mut self) {
        // Safety: Having ownership of `vec`.
        let p: &mut NonEmptyFocusedVec<T> = unsafe { &mut *self.p };
        p.focus = self.focus;

        p.assert_invariant();
    }
}

impl<T> NonEmptyFocusedVecUpdateGuard<'_, T> {
    pub fn commit(self) {
        drop(self);
    }

    pub fn mod_plus_focused_index(&self, diff: isize) -> usize {
        mod_plus_focused_index(self.vec.len(), self.focus, diff)
    }
}

#[inline]
fn mod_plus_focused_index(m: usize, i: usize, diff: isize) -> usize {
    let n: isize = m.try_into().unwrap();
    let i = i as isize;
    (i + diff).rem_euclid(n) as usize
}
