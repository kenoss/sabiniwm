use crate::util::{FocusedVec, Id, NonEmptyFocusedVec};
use crate::view::api::{ViewHandleMessageApi, ViewLayoutApi};
use crate::view::layout_node::{LayoutMessage, LayoutNode, LayoutTreeBuilder};
use crate::view::predefined::{
    LayoutFull, LayoutNodeBorder, LayoutNodeMargin, LayoutNodeSelect, LayoutNodeToggle, LayoutTall,
};
use crate::view::stackset::{StackSet, WorkspaceTag};
use crate::view::window::{Border, Rgba, Window, WindowProps};
use itertools::Itertools;
use smithay::utils::{Logical, Rectangle, Size};
use std::collections::{HashMap, HashSet};

pub struct View {
    // TODO: Avoid internal struct if possible.
    state: ViewState,
}

pub(super) struct ViewState {
    pub(super) stackset: StackSet,
    // TODO: Rename.
    pub(super) layout_queue: Vec<(Id<Window>, WindowProps)>,
    pub(super) windows: HashMap<Id<Window>, Window>,
    pub(super) rect: Rectangle<i32, Logical>,
}

impl View {
    pub fn new(rect: Rectangle<i32, Logical>, workspace_tags: Vec<WorkspaceTag>) -> Self {
        let mut nodes = HashMap::new();

        let node = LayoutNode::from(LayoutTall {});
        let node_id0 = node.id();
        nodes.insert(node_id0, node);

        let node = LayoutNode::from(LayoutFull {});
        let node_id1 = node.id();
        nodes.insert(node_id1, node);

        let layouts = NonEmptyFocusedVec::new(vec![node_id0, node_id1], 0);
        let node = LayoutNode::from(LayoutNodeSelect::new(layouts));
        let node_id = node.id();
        nodes.insert(node_id, node);

        let margin = 8.into();
        let node = LayoutNode::from(LayoutNodeMargin::new(node_id, margin));
        let node_id = node.id();
        nodes.insert(node_id, node);

        let border = Border {
            dim: 2.into(),
            active_rgba: Rgba::from_rgba(0x556b2fff),
            inactive_rgba: Rgba::from_rgba(0x00000000),
        };
        let node = LayoutNode::from(LayoutNodeBorder::new(node_id, border));
        let node_id = node.id();
        nodes.insert(node_id, node);

        let node = LayoutNode::from(LayoutFull {});
        let node_id_full = node.id();
        nodes.insert(node_id_full, node);

        let node = LayoutNode::from(LayoutNodeToggle::new(node_id, node_id_full));
        let node_id = node.id();
        nodes.insert(node_id, node);

        let layout_tree_builder = LayoutTreeBuilder::new(nodes, node_id);

        let stackset = StackSet::new(workspace_tags, layout_tree_builder);

        let state = ViewState {
            stackset,
            layout_queue: Vec::new(),
            windows: HashMap::new(),
            rect,
        };
        Self { state }
    }

    pub fn startup(&mut self) {
        // TODO: Remove?
    }

    pub fn stackset(&self) -> &StackSet {
        &self.state.stackset
    }

    pub fn window(&self, window_id: Id<Window>) -> Option<&Window> {
        self.state.windows.get(&window_id)
    }

    // Returns true iff self is changed.
    pub fn refresh(&mut self, space: &mut smithay::desktop::Space<Window>) -> bool {
        use smithay::utils::IsAlive;

        let mut removed_window_ids = None;
        for window in self.state.windows.values() {
            if !window.alive() {
                if removed_window_ids.is_none() {
                    removed_window_ids = Some(vec![]);
                }

                removed_window_ids.as_mut().unwrap().push(window.id());
            }
        }
        let Some(removed_window_ids) = removed_window_ids else {
            return false;
        };

        let removed_windows = removed_window_ids
            .iter()
            .map(|wid| self.state.windows.remove(wid).unwrap())
            .collect_vec();

        // Speed: In normal use cases, we expect `removed_window_ids.len()` is very small and avoid using `HashSet`.
        //
        // TODO: Support other focus policies, e.g. seeing backforward first.
        let calc_focus = |stack: &FocusedVec<Id<Window>>, i: usize| -> Option<Id<Window>> {
            debug_assert!(i < stack.len() || i == 0);

            let tail = &stack.as_vec()[i..];
            if let Some(j) = tail
                .iter()
                .position(|wid| !removed_window_ids.contains(wid))
            {
                return Some(tail[j]);
            }
            let head = &stack.as_vec()[..i];
            if let Some(k) = head
                .iter()
                .rev()
                .position(|wid| !removed_window_ids.contains(wid))
            {
                return Some(head[i - 1 - k]);
            }
            None
        };
        for workspace in self.state.stackset.workspaces.as_mut().vec.iter_mut() {
            let focus = calc_focus(&workspace.stack, workspace.stack.focused_index());
            let mut stack = workspace.stack.as_mut();
            stack.vec.retain(|wid| !removed_window_ids.contains(wid));
            stack.focus = focus
                .and_then(|focus| stack.vec.iter().position(|&wid| wid == focus))
                .unwrap_or(0);
            stack.commit();
        }
        for window in removed_windows {
            space.unmap_elem(&window);
        }

        self.layout(space);

        true
    }

    pub fn layout(&mut self, space: &mut smithay::desktop::Space<Window>) {
        use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;

        assert!(self.state.layout_queue.is_empty());

        // Layout
        let workspace = self.state.stackset.workspaces().focus();
        // Safety: `LayoutTree` is not borrowed in `ViewLayoutApi`.
        let layout_tree = unsafe { workspace.borrow_layout_tree() };
        let rect = self.state.rect;
        let mut api = ViewLayoutApi {
            workspace,
            layout_tree,
            layout_queue: &mut self.state.layout_queue,
            rect,
        };
        api.layout_node_root();

        // Remove windows from the space that are not in layout result.
        let mut removing_window_ids = space.elements().map(|w| w.id()).collect::<HashSet<_>>();
        for (window_id, _) in &self.state.layout_queue {
            removing_window_ids.remove(window_id);
        }
        for window_id in removing_window_ids {
            let window = self.state.windows.get(&window_id).unwrap();
            space.unmap_elem(window);
        }

        debug!("layout_queue = {:?}", self.state.layout_queue);
        // Reflect layout to the space and surfaces.
        for (window_id, props) in self.state.layout_queue.drain(..) {
            let window = self.state.windows.get_mut(&window_id).unwrap();
            let geometry = props.geometry;
            window.set_props(props);
            space.map_element(window.clone(), geometry.loc, false);
            let Some(surface) = window.toplevel() else {
                continue;
            };
            surface.with_pending_state(|state| {
                state.states.set(xdg_toplevel::State::Fullscreen);
                state.states.set(xdg_toplevel::State::TiledTop);
                state.states.set(xdg_toplevel::State::TiledLeft);
                state.states.set(xdg_toplevel::State::TiledBottom);
                state.states.set(xdg_toplevel::State::TiledRight);
                state.size = Some(geometry.size);
            });
            surface.send_pending_configure();
        }

        assert!(self.state.layout_queue.is_empty());
    }

    pub fn handle_layout_message(
        &mut self,
        message: &LayoutMessage,
        space: &mut smithay::desktop::Space<Window>,
    ) {
        let workspace = self.state.stackset.workspaces().focus();
        // Safety: `LayoutTree` is not borrowed in `ViewLayoutApi`.
        let layout_tree = unsafe { workspace.borrow_layout_tree() };
        let mut api = ViewHandleMessageApi { layout_tree };
        api.handle_message_root(message);

        self.layout(space);
    }

    pub fn resize_output(
        &mut self,
        size: Size<i32, Logical>,
        space: &mut smithay::desktop::Space<Window>,
    ) {
        self.state.rect = Rectangle::from_loc_and_size((0, 0), size);
        self.layout(space);
    }

    pub fn register_window(&mut self, smithay_window: smithay::desktop::Window) -> Id<Window> {
        let window = Window::new(smithay_window);
        let window_id = window.id();
        self.state
            .stackset
            .workspaces
            .focus_mut()
            .stack
            .push(window_id);
        self.state.windows.insert(window_id, window);

        window_id
    }

    pub fn set_focus(&mut self, id: Id<Window>) {
        let workspaces = &mut self.state.stackset.workspaces;

        let mut indice = None;
        for (i, ws) in workspaces.as_vec().iter().enumerate() {
            for (j, &window_id) in ws.stack.as_vec().iter().enumerate() {
                if window_id == id {
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
    }

    pub fn focused_window(&self) -> Option<&Window> {
        self.state
            .stackset
            .workspaces
            .focus()
            .stack
            .focus()
            .map(|id| self.state.windows.get(id).unwrap())
    }

    pub fn focused_window_mut(&mut self) -> Option<&mut Window> {
        self.state
            .stackset
            .workspaces
            .focus()
            .stack
            .focus()
            .map(|id| self.state.windows.get_mut(id).unwrap())
    }

    pub fn update_stackset_with(&mut self, f: impl FnOnce(&mut StackSet)) {
        f(&mut self.state.stackset);
    }
}
