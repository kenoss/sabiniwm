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
        assert!(vec.is_empty() || focus < vec.len());

        Self { vec, focus }
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
    pub fn focused_index_mut(&mut self) -> &mut usize {
        &mut self.focus
    }
}

impl<T> NonEmptyFocusedVec<T> {
    #[inline]
    pub fn new(vec: Vec<T>, focus: usize) -> Self {
        assert!(!vec.is_empty());
        assert!(focus < vec.len());

        Self { vec, focus }
    }

    #[inline]
    pub fn as_vec(&self) -> &Vec<T> {
        &self.vec
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
    pub fn focused_index_mut(&mut self) -> &mut usize {
        &mut self.focus
    }
}