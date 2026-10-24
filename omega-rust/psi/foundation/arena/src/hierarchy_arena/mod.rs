//! Parents own one contiguous child range in the shared dense [`Arena`].
//!
//! Child ranges are contiguous by construction: `insert_children` appends one
//! span and the parent records it, so children resolve through the arena's own
//! span validation instead of a second paged layout. Construction and later
//! extension run in the same arena, so a hierarchy built in place is already
//! published: no node is copied to publish it, and authored handles stay
//! stable because the arena only appends.

use crate::{Arena, Handle, HandleSpan, SpanHandles};

pub trait HierarchyNode: Clone + Default + Sized {
    fn parent(&self) -> Handle<Self>;
    fn set_parent(&mut self, parent: Handle<Self>);
    fn children(&self) -> HandleSpan<Self>;
    fn set_children(&mut self, children: HandleSpan<Self>);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HierarchyArena<T: HierarchyNode> {
    nodes: Arena<T>,
}

impl<T: HierarchyNode> HierarchyArena<T> {
    pub fn new() -> Self {
        Self {
            nodes: Arena::new(),
        }
    }

    #[inline]
    pub fn get(&self, node: Handle<T>) -> &T {
        self.nodes.get(node)
    }

    /// Mutate one node's payload. Parent and child ranges stay as built.
    pub fn get_mut(&mut self, node: Handle<T>) -> &mut T {
        self.nodes.get_mut(node)
    }

    /// Insert one root. A root has no parent and no child range.
    pub fn insert_root(&mut self, mut node: T) -> Handle<T> {
        node.set_parent(Handle::invalid());
        node.set_children(HandleSpan::empty());

        self.nodes.append(node)
    }

    /// Insert one parent's complete child range as a contiguous span. The
    /// parent must be an existing node that does not own a child range yet;
    /// this deliberately does not mutate an authored range or attempt a
    /// non-contiguous append.
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
            self.get(parent).children().is_empty(),
            "hierarchy arena parent already has a child range"
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

    /// Append one child without extending the parent's frozen contiguous
    /// child range. Callers must retain the returned handle in a companion
    /// index and include it in lookup explicitly.
    pub fn insert_supplemental_child(&mut self, parent: Handle<T>, mut node: T) -> Handle<T> {
        node.set_parent(parent);
        node.set_children(HandleSpan::empty());

        self.nodes.append(node)
    }

    /// The parent's child range, resolved as one contiguous slice.
    pub fn children(&self, parent: Handle<T>) -> Option<&[T]> {
        self.nodes.span(self.get(parent).children())
    }

    /// The handles of one parent's child range, validated like the range
    /// itself. An absent range occupies no handles.
    pub fn child_handles(&self, parent: Handle<T>) -> Option<SpanHandles<T>> {
        let children = self.get(parent).children();

        if children.is_empty() {
            return Some(SpanHandles::empty());
        }

        self.nodes.span(children)?;

        Some(children.handles())
    }

    pub fn find_child(
        &self,
        parent: Handle<T>,
        mut matches: impl FnMut(Handle<T>, &T) -> bool,
    ) -> Option<Handle<T>> {
        self.child_handles(parent)?
            .find(|&child| matches(child, self.get(child)))
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn nodes(&self) -> &Arena<T> {
        &self.nodes
    }
}

impl<T: HierarchyNode> Default for HierarchyArena<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use crate::{Handle, HandleSpan, HierarchyArena, HierarchyNode};

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
    fn construction_sets_parent_and_exact_child_range() {
        let mut arena = HierarchyArena::new();
        let root = arena.insert_root(TestNode::named("root"));
        let children =
            arena.insert_children(root, [TestNode::named("alpha"), TestNode::named("beta")]);

        assert_eq!(arena.get(root).children(), children);
        assert_eq!(children.start().arena_index(), 2);
        assert_eq!(children.count(), 2);
        assert_eq!(
            arena.children(root).expect("children should resolve")[0].name,
            "alpha"
        );
        assert_eq!(
            arena.children(root).expect("children should resolve")[1].parent(),
            root
        );
    }

    #[test]
    fn bulk_child_range_resolves_as_one_slice() {
        let mut arena = HierarchyArena::new();
        let root = arena.insert_root(TestNode::named("root"));
        let children = arena.insert_children(root, (0..600).map(|_| TestNode::named("child")));

        // Appends only ever grow the dense arena, so a range larger than any
        // former page size still resolves through one span.
        let resolved = arena.children(root).expect("children should resolve");
        assert_eq!(resolved.len(), 600);
        assert_eq!(children.count(), 600);
        assert_eq!(
            arena
                .child_handles(root)
                .expect("children should resolve")
                .count(),
            600
        );
    }

    #[test]
    fn walks_child_handles() {
        let mut arena = HierarchyArena::new();
        let root = arena.insert_root(TestNode::named("root"));
        arena.insert_children(root, [TestNode::named("alpha"), TestNode::named("beta")]);

        let child_names = arena
            .child_handles(root)
            .expect("children should resolve")
            .map(|child| arena.get(child).name)
            .collect::<Vec<_>>();

        assert_eq!(child_names, vec!["alpha", "beta"]);
    }

    #[test]
    fn child_handles_skip_by_index_and_stay_exhausted() {
        let mut arena = HierarchyArena::new();
        let root = arena.insert_root(TestNode::named("root"));
        arena.insert_children(root, [TestNode::named("alpha"), TestNode::named("beta")]);

        let mut children = arena.child_handles(root).expect("children should resolve");
        assert_eq!(children.len(), 2);
        let second = children.nth(1).expect("second child");
        assert_eq!(arena.get(second).name, "beta");
        assert_eq!(children.len(), 0);
        assert_eq!(children.nth(0), None);
        assert_eq!(children.next(), None);
    }

    #[test]
    fn finds_child_by_local_sibling_scan() {
        let mut arena = HierarchyArena::new();
        let root = arena.insert_root(TestNode::named("root"));
        arena.insert_children(root, [TestNode::named("alpha"), TestNode::named("beta")]);

        let beta = arena
            .find_child(root, |_, node| node.name == "beta")
            .expect("beta should resolve");

        assert_eq!(arena.get(beta).name, "beta");
        assert_eq!(arena.get(beta).parent(), root);
    }

    #[test]
    fn extends_a_retained_hierarchy_without_rebinding_authored_handles() {
        let mut arena = HierarchyArena::new();
        let root = arena.insert_root(TestNode::named("root"));
        let authored = arena.insert_children(root, [TestNode::named("alpha")]);

        let generated = arena.insert_root(TestNode::named("generated"));
        let supplemental = arena.insert_supplemental_child(root, TestNode::named("late"));

        assert_eq!(arena.get(authored.start()).name, "alpha");
        assert_eq!(arena.get(root).children(), authored);
        assert_eq!(
            arena.children(root).expect("authored range")[0].name,
            "alpha"
        );
        assert_eq!(arena.get(supplemental).parent(), root);
        assert_eq!(arena.len(), 4);
        assert_eq!(arena.nodes().len(), 4);
        assert_eq!(arena.get(generated).parent(), Handle::invalid());
    }

    #[test]
    fn rejects_duplicate_child_ranges() {
        let mut arena = HierarchyArena::new();
        let root = arena.insert_root(TestNode::named("root"));
        arena.insert_children(root, [TestNode::named("alpha")]);

        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            arena.insert_children(root, [TestNode::named("beta")]);
        }));

        assert!(panic.is_err());
    }
}
