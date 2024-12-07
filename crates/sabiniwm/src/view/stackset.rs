use crate::util::{FocusedVec, Id, NonEmptyFocusedVec};
use crate::view::layout_node::{LayoutTree, LayoutTreeBuilder};
use crate::view::window::Window;
use std::cell::UnsafeCell;

pub struct StackSet {
    pub workspaces: NonEmptyFocusedVec<Workspace>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceTag(pub String);

pub struct Workspace {
    pub tag: WorkspaceTag,
    pub stack: FocusedVec<Id<Window>>,
    layout_tree: UnsafeCell<LayoutTree>,
}

impl StackSet {
    pub(super) fn new(tags: Vec<WorkspaceTag>, layout_tree_builder: LayoutTreeBuilder) -> Self {
        let workspaces = tags
            .into_iter()
            .map(|tag| Workspace {
                tag,
                stack: FocusedVec::default(),
                layout_tree: UnsafeCell::new(layout_tree_builder.clone().build()),
            })
            .collect();
        let workspaces = NonEmptyFocusedVec::new(workspaces, 0);
        Self { workspaces }
    }

    pub fn workspaces(&self) -> &NonEmptyFocusedVec<Workspace> {
        &self.workspaces
    }
}

impl Workspace {
    pub fn stack(&self) -> &FocusedVec<Id<Window>> {
        &self.stack
    }

    #[allow(clippy::mut_from_ref)]
    pub(super) unsafe fn borrow_layout_tree(&self) -> &mut LayoutTree {
        &mut *self.layout_tree.get()
    }
}
