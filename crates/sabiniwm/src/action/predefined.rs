use crate::action::action::{Action, ActionFnI};
use crate::backend::BackendI;
use crate::state::SabiniwmState;
use crate::view::stackset::WorkspaceTag;

#[derive(Debug, Clone)]
pub struct ActionWithSavedFocus(pub Action);

impl ActionFnI for ActionWithSavedFocus {
    fn exec(&self, state: &mut SabiniwmState) {
        // TODO: Save window focus.

        let ss = state.inner.view.stackset();
        let ws_index = ss.workspaces.focused_index();

        state.process_action(&self.0);

        state.inner.view.update_stackset_with(|stackset| {
            stackset.workspaces.set_focused_index(ws_index);
        });
    }
}

#[derive(Debug, Clone)]
pub struct ActionQuitSabiniwm;

impl ActionFnI for ActionQuitSabiniwm {
    fn exec(&self, state: &mut SabiniwmState) {
        state.inner.loop_signal.stop();
    }
}

#[derive(Debug, Clone)]
pub struct ActionChangeVt(pub i32);

impl ActionFnI for ActionChangeVt {
    fn exec(&self, state: &mut SabiniwmState) {
        state.backend.change_vt(self.0);
    }
}

#[derive(Debug, Clone)]
pub enum ActionMoveFocus {
    Next,
    Prev,
}

impl ActionFnI for ActionMoveFocus {
    fn exec(&self, state: &mut SabiniwmState) {
        let count = match self {
            Self::Next => 1,
            Self::Prev => -1,
        };
        state.inner.view.update_stackset_with(|stackset| {
            let stack = &mut stackset.workspaces.focus_mut().stack;
            let i = stack.mod_plus_focused_index(count);
            stack.set_focused_index(i);
        });
    }
}

#[derive(Debug, Clone)]
pub enum ActionWindowSwap {
    Next,
    Prev,
}

impl ActionFnI for ActionWindowSwap {
    fn exec(&self, state: &mut SabiniwmState) {
        let count = match self {
            Self::Next => 1,
            Self::Prev => -1,
        };
        state.inner.view.update_stackset_with(|stackset| {
            let stack = &mut stackset.workspaces.focus_mut().stack;

            if stack.is_empty() {
                return;
            }

            let mut stack = stack.as_mut();
            let i = stack.focus;
            let j = stack.mod_plus_focused_index(count);
            stack.vec.swap(i, j);
            stack.focus = j;
            stack.commit();
        });
    }
}

#[derive(Debug, Clone)]
pub enum ActionWorkspaceFocus {
    Next,
    Prev,
    WithTag(WorkspaceTag),
}

impl ActionFnI for ActionWorkspaceFocus {
    fn exec(&self, state: &mut SabiniwmState) {
        let count = match self {
            Self::Next => 1,
            Self::Prev => -1,
            Self::WithTag(tag) => {
                let ss = state.inner.view.stackset();
                let src = ss.workspaces.focused_index();
                // TODO: Error handling.
                let dst = ss
                    .workspaces
                    .as_vec()
                    .iter()
                    .position(|ws| ws.tag == *tag)
                    .expect("workspace with the given tag exists");
                dst as isize - src as isize
            }
        };
        state.inner.view.update_stackset_with(|stackset| {
            let workspaces = &mut stackset.workspaces;
            let i = workspaces.mod_plus_focused_index(count);
            workspaces.set_focused_index(i);
        });
    }
}

#[derive(Debug, Clone)]
pub enum ActionWorkspaceFocusNonEmpty {
    Next,
    Prev,
}

impl ActionFnI for ActionWorkspaceFocusNonEmpty {
    fn exec(&self, state: &mut SabiniwmState) {
        let direction = match self {
            Self::Next => 1,
            Self::Prev => -1,
        };
        state.inner.view.update_stackset_with(|stackset| {
            let workspaces = &mut stackset.workspaces;
            for d in 1..workspaces.len() {
                let i = workspaces.mod_plus_focused_index(direction * d as isize);
                if !workspaces.as_vec()[i].stack.is_empty() {
                    workspaces.set_focused_index(i);
                    return;
                }
            }
        });
    }
}

#[derive(Debug, Clone)]
pub enum ActionWindowMoveToWorkspace {
    Next,
    Prev,
    WithTag(WorkspaceTag),
}

impl ActionFnI for ActionWindowMoveToWorkspace {
    fn exec(&self, state: &mut SabiniwmState) {
        let count = match self {
            Self::Next => 1,
            Self::Prev => -1,
            Self::WithTag(tag) => {
                let ss = state.inner.view.stackset();
                let src = ss.workspaces.focused_index();
                // TODO: Error handling.
                let dst = ss
                    .workspaces
                    .as_vec()
                    .iter()
                    .position(|ws| ws.tag == *tag)
                    .expect("workspace with the given tag exists");
                dst as isize - src as isize
            }
        };
        state.inner.view.update_stackset_with(|stackset| {
            let mut workspaces = stackset.workspaces.as_mut();

            let mut src = workspaces.vec[workspaces.focus].stack.as_mut();
            let window = src.vec.remove(src.focus);
            src.focus = src.focus.min(src.vec.len().saturating_sub(1));
            src.commit();

            workspaces.focus = workspaces.mod_plus_focused_index(count);

            let dst = workspaces.vec[workspaces.focus].stack.as_mut();
            dst.vec.insert(dst.focus, window);
            dst.commit();

            workspaces.commit();
        });
    }
}

#[derive(Debug, Clone)]
pub struct ActionWindowKill {}

impl ActionFnI for ActionWindowKill {
    fn exec(&self, state: &mut SabiniwmState) {
        use smithay::desktop::WindowSurface;

        let Some(window) = state.inner.view.focused_window_mut() else {
            return;
        };

        match window.smithay_window().underlying_surface() {
            WindowSurface::Wayland(w) => w.send_close(),
            WindowSurface::X11(w) => {
                let _ = w.close();
            }
        };
    }
}

#[derive(Debug, Clone)]
pub struct ActionWindowFloat {}

impl ActionFnI for ActionWindowFloat {
    fn exec(&self, state: &mut SabiniwmState) {
        let Some(window) = state.inner.view.focused_window() else {
            return;
        };
        state.inner.view.make_window_float(window.id());
    }
}

#[derive(Debug, Clone)]
pub struct ActionWindowSink {}

impl ActionFnI for ActionWindowSink {
    fn exec(&self, state: &mut SabiniwmState) {
        use crate::view::stackset::WindowFocusType;

        let window_id = state.inner.view.update_stackset_with(|stackset| {
            if stackset.window_focus_type == WindowFocusType::Stack {
                return None;
            }

            let fw = stackset.float_windows.pop().unwrap(/* must not be empty as WindowFocusType is Float */);
            // WindowFocusType will be updated in `set_focus()` if needed.

            let workspaces = stackset.workspaces.as_mut();

            let dst = workspaces.vec[workspaces.focus].stack.as_mut();
            dst.vec.insert(0, fw.id);
            dst.commit();

            Some(fw.id)
        });
        if let Some(window_id) = window_id {
            state.inner.view.set_focus(window_id);
        }
    }
}
