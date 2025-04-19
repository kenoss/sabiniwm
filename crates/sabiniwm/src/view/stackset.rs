use crate::util::{FocusedVec, Id, NonEmptyFocusedVec};
use crate::view::layout_node::{LayoutTree, LayoutTreeBuilder};
use crate::view::window::Window;
use smithay::utils::{Logical, Rectangle};
use std::cell::UnsafeCell;

pub struct StackSet {
    pub workspaces: NonEmptyFocusedVec<Workspace>,
    // Bottom to top (because grab removes/inserts the top element and we make it O(1)).
    pub float_windows: Vec<FloatWindow>,
    pub window_focus_type: WindowFocusType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceTag(pub String);

pub struct Workspace {
    pub tag: WorkspaceTag,
    pub stack: FocusedVec<Id<Window>>,
    layout_tree: UnsafeCell<LayoutTree>,
}

pub struct FloatWindow {
    pub id: Id<Window>,
    pub geometry: Rectangle<i32, Logical>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowFocusType {
    // `StackSet::workspaces.focus().stack.focus()` is focused if `Some`.
    Stack,
    // `StackSet::workspaces.float_windows.last().unwrap()` is focused.
    Float,
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
        Self {
            workspaces,
            float_windows: vec![],
            window_focus_type: WindowFocusType::Stack,
        }
    }

    pub fn workspaces(&self) -> &NonEmptyFocusedVec<Workspace> {
        &self.workspaces
    }

    pub fn set_focus(&mut self, window_id: Id<Window>) {
        let i = self.float_windows.iter().position(|x| x.id == window_id);
        if let Some(i) = i {
            let fw = self.float_windows.remove(i);
            self.float_windows.push(fw);

            self.window_focus_type = WindowFocusType::Float;
        } else {
            let workspaces = &mut self.workspaces;

            let mut indice = None;
            for (i, ws) in workspaces.as_vec().iter().enumerate() {
                for (j, &wid) in ws.stack.as_vec().iter().enumerate() {
                    if wid == window_id {
                        indice = Some((i, j));
                        break;
                    }
                }
            }
            let Some((i, j)) = indice else {
                return;
            };

            workspaces.set_focused_index(i);
            workspaces.focus_mut().stack.set_focused_index(j);

            self.window_focus_type = WindowFocusType::Stack;
        }
    }

    pub fn delete_window(&mut self, window_id: Id<Window>) {
        let workspaces = self.workspaces.as_mut();

        for workspace in workspaces.vec.iter_mut() {
            let mut stack = workspace.stack.as_mut();
            if let Some(i) = stack.vec.iter().position(|&wid| wid == window_id) {
                stack.vec.remove(i);
                stack.focus = stack.focus.min(stack.vec.len().saturating_sub(1));
                stack.commit();

                return;
            }
        }

        if let Some(i) = self.float_windows.iter().position(|fw| fw.id == window_id) {
            self.float_windows.remove(i);
        }
    }

    pub(crate) fn make_window_float(
        &mut self,
        window_id: Id<Window>,
        geometry: Rectangle<i32, Logical>,
    ) {
        // `delete_window()` returning `FloatWindow`.
        let fw = (|| {
            let workspaces = self.workspaces.as_mut();

            for workspace in workspaces.vec.iter_mut() {
                let mut stack = workspace.stack.as_mut();
                if let Some(i) = stack.vec.iter().position(|&wid| wid == window_id) {
                    stack.vec.remove(i);
                    stack.focus = stack.focus.min(stack.vec.len().saturating_sub(1));
                    stack.commit();

                    return Some(FloatWindow {
                        id: window_id,
                        geometry,
                    });
                }
            }

            if let Some(i) = self.float_windows.iter().position(|fw| fw.id == window_id) {
                return Some(self.float_windows.remove(i));
            }

            None
        })();
        let fw = fw.unwrap();

        self.float_windows.push(fw);
        self.set_focus(window_id);
    }

    pub fn float_window_with_rect(&mut self, window_id: Id<Window>, rect: Rectangle<i32, Logical>) {
        let fw = (|| {
            let workspaces = self.workspaces.as_mut();

            for workspace in workspaces.vec.iter_mut() {
                let mut stack = workspace.stack.as_mut();
                if let Some(i) = stack.vec.iter().position(|&wid| wid == window_id) {
                    stack.vec.remove(i);
                    stack.focus = stack.focus.min(stack.vec.len().saturating_sub(1));
                    stack.commit();

                    return Some(FloatWindow {
                        id: window_id,
                        geometry: rect,
                    });
                }
            }

            if let Some(i) = self.float_windows.iter().position(|fw| fw.id == window_id) {
                return Some(self.float_windows.remove(i));
            }

            None
        })();
        let fw = fw.unwrap();

        self.float_windows.push(fw);
        self.set_focus(window_id);
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
