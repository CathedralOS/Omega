use std::marker::PhantomData;

use crate::{Arena, Handle, HandleSpan, PagedArena, PagedSlice};

pub trait HierarchyNode: Clone + Default + Sized {
    fn parent(&self) -> Handle<Self>;
    fn set_parent(&mut self, parent: Handle<Self>);
    fn children(&self) -> HandleSpan<Self>;
    fn set_children(&mut self, children: HandleSpan<Self>);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HierarchyArena<T: HierarchyNode> {
    nodes: PagedArena<T>,
}

impl<T: HierarchyNode> HierarchyArena<T> {
    pub fn get(&self, node: Handle<T>) -> &T {
        self.nodes.get(node)
    }

    /// Mutate metadata on an existing node while preserving the hierarchy.
    pub fn update_nonstructural(&mut self, node: Handle<T>, update: impl FnOnce(&mut T)) {
        let parent = self.nodes.get(node).parent();
        let children = self.nodes.get(node).children();
        update(self.nodes.get_mut(node));
        assert!(
            self.nodes.get(node).parent() == parent && self.nodes.get(node).children() == children,
            "non-structural hierarchy update changed parent/child topology"
        );
    }

    /// Insert a generated root after the authored hierarchy has been frozen.
    /// Later compiler stages use this for materialized declarations (for
    /// example generic-machine specializations). Authored handles remain
    /// stable because the paged arena only appends.
    pub fn insert_generated_root(&mut self, mut node: T) -> Handle<T> {
        node.set_parent(Handle::invalid());
        node.set_children(HandleSpan::empty());
        self.nodes.insert(node)
    }

    /// Populate one freshly generated parent's child range. The parent must
    /// not already own children; this deliberately does not mutate authored
    /// ranges or attempt non-contiguous append.
    pub fn insert_generated_children(
        &mut self,
        parent: Handle<T>,
        children: impl IntoIterator<Item = T>,
    ) -> HandleSpan<T> {
        assert!(
            self.get(parent).children().is_empty(),
            "generated hierarchy parent already has children"
        );
        let children = self
            .nodes
            .insert_many(children.into_iter().map(|mut child| {
                child.set_parent(parent);
                child.set_children(HandleSpan::empty());
                child
            }));
        self.nodes.get_mut(parent).set_children(children);
        children
    }

    pub fn children(&self, parent: Handle<T>) -> Option<PagedSlice<'_, T>> {
        let parent_node = self.get(parent);

        self.nodes.paged_span(parent_node.children())
    }

    pub fn child_handles(&self, parent: Handle<T>) -> Option<HierarchyChildHandles<T>> {
        let parent_node = self.get(parent);
        let children = parent_node.children();

        if children.is_empty() {
            return Some(HierarchyChildHandles::empty());
        }

        if self.nodes.paged_span(children).is_none() {
            return None;
        }

        Some(HierarchyChildHandles {
            next_index: children.start().arena_index(),
            remaining: children.count(),
            marker: PhantomData,
        })
    }

    pub fn find_child(
        &self,
        parent: Handle<T>,
        mut matches: impl FnMut(Handle<T>, &T) -> bool,
    ) -> Option<Handle<T>> {
        for child in self.child_handles(parent)? {
            if matches(child, self.get(child)) {
                return Some(child);
            }
        }

        None
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn nodes(&self) -> &PagedArena<T> {
        &self.nodes
    }
}

impl<T: HierarchyNode> Default for HierarchyArena<T> {
    fn default() -> Self {
        Self {
            nodes: PagedArena::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HierarchyArenaBuilder<T: HierarchyNode> {
    nodes: Arena<T>,
}

impl<T: HierarchyNode> HierarchyArenaBuilder<T> {
    pub fn new() -> Self {
        Self {
            nodes: Arena::new(),
        }
    }

    pub fn insert_root(&mut self, mut node: T) -> Handle<T> {
        node.set_parent(Handle::invalid());
        node.set_children(HandleSpan::empty());

        self.nodes.insert(node)
    }

    pub fn insert_child(&mut self, parent: Handle<T>, mut node: T) -> Handle<T> {
        node.set_parent(parent);
        node.set_children(HandleSpan::empty());

        self.nodes.insert(node)
    }

    pub fn insert_children(
        &mut self,
        parent: Handle<T>,
        children: impl IntoIterator<Item = T>,
    ) -> HandleSpan<T> {
        assert!(
            self.nodes.is_valid(parent),
            "hierarchy arena parent handle must be valid"
        );
        assert!(
            self.nodes.get(parent).children().is_empty(),
            "hierarchy arena parent already has a child range"
        );

        let mut start = Handle::invalid();
        let mut count = 0u32;

        for mut child in children {
            child.set_parent(parent);
            child.set_children(HandleSpan::empty());

            let child = self.nodes.append(child);
            if count == 0 {
                start = child;
            }
            count = count
                .checked_add(1)
                .expect("hierarchy child span count overflow");
        }

        let span = if count == 0 {
            HandleSpan::empty()
        } else {
            HandleSpan::from_parts(start, count)
        };

        self.nodes.get_mut(parent).set_children(span);

        span
    }

    pub fn get(&self, node: Handle<T>) -> &T {
        self.nodes.get(node)
    }

    pub fn get_mut(&mut self, node: Handle<T>) -> &mut T {
        self.nodes.get_mut(node)
    }

    pub fn children(&self, parent: Handle<T>) -> Option<&[T]> {
        let parent_node = self.get(parent);

        self.nodes.span(parent_node.children())
    }

    pub fn finish(self) -> HierarchyArena<T> {
        let mut nodes = PagedArena::new();

        nodes.insert_many(self.nodes.iter().map(|(_, node)| node.clone()));

        HierarchyArena { nodes }
    }
}

impl<T: HierarchyNode> Default for HierarchyArenaBuilder<T> {
    fn default() -> Self {
        Self::new()
    }
}

pub struct HierarchyChildHandles<T> {
    next_index: u32,
    remaining: u32,
    marker: PhantomData<fn() -> T>,
}

impl<T> HierarchyChildHandles<T> {
    fn empty() -> Self {
        Self {
            next_index: 0,
            remaining: 0,
            marker: PhantomData,
        }
    }
}

impl<T> Iterator for HierarchyChildHandles<T> {
    type Item = Handle<T>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }

        let handle = Handle::from_arena_index(self.next_index);
        self.next_index = self
            .next_index
            .checked_add(1)
            .expect("hierarchy child handle index overflow");
        self.remaining -= 1;

        Some(handle)
    }
}

#[cfg(test)]
mod tests {
    use crate::{Handle, HandleSpan, HierarchyArenaBuilder, HierarchyNode};

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    struct TestNode {
        parent: Handle<TestNode>,
        children: HandleSpan<TestNode>,
        name: &'static str,
    }

    impl TestNode {
        fn named(name: &'static str) -> Self {
            Self {
                name,
                ..Self::default()
            }
        }
    }

    impl HierarchyNode for TestNode {
        fn parent(&self) -> Handle<Self> {
            self.parent
        }

        fn set_parent(&mut self, parent: Handle<Self>) {
            self.parent = parent;
        }

        fn children(&self) -> HandleSpan<Self> {
            self.children
        }

        fn set_children(&mut self, children: HandleSpan<Self>) {
            self.children = children;
        }
    }

    #[test]
    fn builder_sets_parent_and_exact_child_range() {
        let mut builder = HierarchyArenaBuilder::new();
        let root = builder.insert_root(TestNode::named("root"));
        let children =
            builder.insert_children(root, [TestNode::named("alpha"), TestNode::named("beta")]);

        assert_eq!(builder.get(root).children(), children);
        assert_eq!(children.start().arena_index(), 2);
        assert_eq!(children.count(), 2);
        assert_eq!(
            builder.children(root).expect("children should resolve")[0].name,
            "alpha"
        );
        assert_eq!(
            builder.children(root).expect("children should resolve")[1].parent(),
            root
        );
    }

    #[test]
    fn published_arena_walks_child_handles() {
        let mut builder = HierarchyArenaBuilder::new();
        let root = builder.insert_root(TestNode::named("root"));
        builder.insert_children(root, [TestNode::named("alpha"), TestNode::named("beta")]);
        let arena = builder.finish();

        let child_names = arena
            .child_handles(root)
            .expect("children should resolve")
            .map(|child| arena.get(child).name)
            .collect::<Vec<_>>();

        assert_eq!(child_names, vec!["alpha", "beta"]);
    }

    #[test]
    fn finds_child_by_local_sibling_scan() {
        let mut builder = HierarchyArenaBuilder::new();
        let root = builder.insert_root(TestNode::named("root"));
        builder.insert_children(root, [TestNode::named("alpha"), TestNode::named("beta")]);
        let arena = builder.finish();

        let beta = arena
            .find_child(root, |_, node| node.name == "beta")
            .expect("beta should resolve");

        assert_eq!(arena.get(beta).name, "beta");
        assert_eq!(arena.get(beta).parent(), root);
    }

    #[test]
    fn rejects_duplicate_child_ranges() {
        let mut builder = HierarchyArenaBuilder::new();
        let root = builder.insert_root(TestNode::named("root"));
        builder.insert_children(root, [TestNode::named("alpha")]);

        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            builder.insert_children(root, [TestNode::named("beta")]);
        }));

        assert!(panic.is_err());
    }
}
