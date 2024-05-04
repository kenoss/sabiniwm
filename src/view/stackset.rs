use crate::util::Id;
use crate::util::{FocusedVec, NonEmptyFocusedVec};
use crate::view::window::Window;

pub struct StackSet {
    pub(super) workspaces: NonEmptyFocusedVec<Workspace>,
}

pub struct Workspace {
    // tag: String,
    pub(super) stack: FocusedVec<Id<Window>>,
}

impl StackSet {
    pub(super) fn new() -> Self {
        let workspace = Workspace {
            stack: FocusedVec::default(),
        };
        Self {
            workspaces: NonEmptyFocusedVec::new(vec![workspace], 0),
        }
    }

    pub fn workspaces(&self) -> &NonEmptyFocusedVec<Workspace> {
        &self.workspaces
    }
}

impl Workspace {
    pub fn stack(&self) -> &FocusedVec<Id<Window>> {
        &self.stack
    }

    pub fn focus_next_window(&mut self, count: isize) {
        let i = self.stack.mod_plus_focused_index(count);
        self.stack.set_focused_index(i);
    }
}
