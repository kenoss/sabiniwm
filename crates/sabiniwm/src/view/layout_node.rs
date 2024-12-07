use crate::util::Id;
use crate::view::api::{ViewHandleMessageApi, ViewLayoutApi};
use downcast::Any;
use dyn_clone::DynClone;
use std::cell::UnsafeCell;
use std::collections::HashMap;

pub trait LayoutMessageI: Any + std::fmt::Debug + DynClone {}

downcast::downcast!(dyn LayoutMessageI);
dyn_clone::clone_trait_object!(LayoutMessageI);

#[derive(Debug, Clone)]
pub struct LayoutMessage {
    inner: Box<dyn LayoutMessageI>,
}

impl<T> From<T> for LayoutMessage
where
    T: LayoutMessageI,
{
    fn from(x: T) -> Self {
        Self { inner: Box::new(x) }
    }
}

impl LayoutMessage {
    pub fn downcast_ref<T>(&self) -> Option<&T>
    where
        T: LayoutMessageI,
    {
        self.inner.as_ref().downcast_ref().ok()
    }
}

pub trait LayoutNodeI: dyn_clone::DynClone {
    fn layout(&self, api: &mut ViewLayoutApi<'_>);

    // The defalut implementation is for leaf node.
    fn handle_message(
        &mut self,
        _api: &mut ViewHandleMessageApi<'_>,
        _message: &LayoutMessage,
    ) -> std::ops::ControlFlow<()> {
        std::ops::ControlFlow::Continue(())
    }
}

dyn_clone::clone_trait_object!(LayoutNodeI);

#[derive(Clone)]
pub struct LayoutNode {
    id: Id<Self>,
    inner: Box<dyn LayoutNodeI>,
}

impl<T> From<T> for LayoutNode
where
    T: LayoutNodeI + 'static,
{
    fn from(inner: T) -> Self {
        Self {
            id: Id::new(),
            inner: Box::new(inner),
        }
    }
}

impl LayoutNode {
    pub fn id(&self) -> Id<Self> {
        self.id
    }

    pub fn layout(&self, api: &mut ViewLayoutApi<'_>) {
        self.inner.layout(api);
    }

    pub fn handle_message(
        &mut self,
        api: &mut ViewHandleMessageApi<'_>,
        message: &LayoutMessage,
    ) -> std::ops::ControlFlow<()> {
        self.inner.handle_message(api, message)
    }
}

#[derive(Clone)]
pub(super) struct LayoutTreeBuilder {
    nodes: HashMap<Id<LayoutNode>, LayoutNode>,
    root_id: Id<LayoutNode>,
}

pub(super) struct LayoutTree {
    nodes: HashMap<Id<LayoutNode>, UnsafeCell<LayoutNode>>,
    root_id: Id<LayoutNode>,
}

impl LayoutTreeBuilder {
    pub fn new(nodes: HashMap<Id<LayoutNode>, LayoutNode>, root_id: Id<LayoutNode>) -> Self {
        Self { nodes, root_id }
    }

    pub fn build(self) -> LayoutTree {
        let nodes = self
            .nodes
            .into_iter()
            .map(|(id, node)| (id, UnsafeCell::new(node)))
            .collect();
        LayoutTree {
            nodes,
            root_id: self.root_id,
        }
    }
}

impl LayoutTree {
    pub fn root_id(&self) -> Id<LayoutNode> {
        self.root_id
    }

    pub unsafe fn borrow_node<'a>(&self, node_id: Id<LayoutNode>) -> &'a LayoutNode {
        &*self.nodes.get(&node_id).unwrap().get()
    }

    pub unsafe fn borrow_mut_node<'a>(&self, node_id: Id<LayoutNode>) -> &'a mut LayoutNode {
        &mut *self.nodes.get(&node_id).unwrap().get()
    }
}
