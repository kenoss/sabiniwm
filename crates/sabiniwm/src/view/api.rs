use crate::util::Id;
use crate::view::layout_node::{LayoutMessage, LayoutNode, LayoutTree};
use crate::view::stackset::Workspace;
use crate::view::window::{Border, Rgba, Window, WindowProps};
use smithay::utils::{Logical, Rectangle};

pub struct ViewLayoutApi<'a> {
    pub(super) workspace: &'a Workspace,
    pub(super) layout_tree: &'a mut LayoutTree,
    pub(super) layout_queue: &'a mut Vec<(Id<Window>, WindowProps)>,
    pub(super) rect: Rectangle<i32, Logical>,
}

impl ViewLayoutApi<'_> {
    pub fn workspace(&self) -> &Workspace {
        self.workspace
    }

    pub fn rect(&self) -> &Rectangle<i32, Logical> {
        &self.rect
    }

    pub fn layout_node(&mut self, id: Id<LayoutNode>, rect: Rectangle<i32, Logical>) {
        assert!(self.rect.contains_rect(rect));

        // Safety: `LayoutTree` doesn't allow recursive structure.
        let node = unsafe { self.layout_tree.borrow_node(id) };
        let mut api = ViewLayoutApi {
            workspace: self.workspace,
            layout_tree: self.layout_tree,
            layout_queue: self.layout_queue,
            rect,
        };
        node.layout(&mut api);
    }

    pub(super) fn layout_node_root(&mut self) {
        let root_id = self.layout_tree.root_id();
        self.layout_node(root_id, self.rect);
    }

    pub fn layout_window(&mut self, id: Id<Window>, geometry: Rectangle<i32, Logical>) {
        // TODO: Check that id is not already registered.
        let border = Border {
            dim: 0.into(),
            active_rgba: Rgba::from_rgba(0x000000ff),
            inactive_rgba: Rgba::from_rgba(0x000000ff),
        };
        let props = WindowProps { geometry, border };
        self.layout_queue.push((id, props));
    }

    pub fn modify_layout_queue_with<F>(&mut self, f: F)
    where
        F: Fn(&mut Vec<(Id<Window>, WindowProps)>),
    {
        f(self.layout_queue);
    }
}

pub struct ViewHandleMessageApi<'a> {
    pub(super) layout_tree: &'a mut LayoutTree,
}

impl ViewHandleMessageApi<'_> {
    pub fn handle_message(
        &mut self,
        id: Id<LayoutNode>,
        message: &LayoutMessage,
    ) -> std::ops::ControlFlow<()> {
        // Safety: `LayoutTree` doesn't allow recursive structure.
        let node = unsafe { self.layout_tree.borrow_mut_node(id) };
        node.handle_message(self, message)
    }

    pub fn handle_message_root(&mut self, message: &LayoutMessage) {
        let root_id = self.layout_tree.root_id();
        self.handle_message(root_id, message);
    }
}
