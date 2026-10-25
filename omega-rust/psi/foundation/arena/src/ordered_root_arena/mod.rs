use std::marker::PhantomData;
use std::ops::{Index, IndexMut};

use crate::{Arena, Handle, HandleSpan};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderedRootArena<T: Default> {
    handles: Arena<Handle<T>>,
    roots: HandleSpan<Handle<T>>,
    storage: Arena<T>,
}

impl<T: Default> OrderedRootArena<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, value: T) -> Handle<T> {
        let value = self.storage.append(value);
        self.handles.append_to_span(&mut self.roots, value);

        value
    }

    pub fn len(&self) -> usize {
        self.roots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.roots.is_empty()
    }

    pub fn first(&self) -> Option<&T> {
        self.iter().next()
    }

    /// Read an exact identity returned by `push`. Root selection never moves
    /// its declaration storage or reuses an identity for another declaration.
    pub fn get(&self, handle: Handle<T>) -> &T {
        self.storage.get(handle)
    }

    pub fn contains(&self, handle: Handle<T>) -> bool {
        self.handles.span_or_empty(self.roots).contains(&handle)
    }

    /// Select roots without rebuilding their storage. Durable references to
    /// retained roots keep their original generational identities.
    pub fn retain(&mut self, mut keep: impl FnMut(&T) -> bool) {
        let retained = self
            .handles
            .span_or_empty(self.roots)
            .iter()
            .copied()
            .filter(|handle| keep(self.storage.get(*handle)))
            .collect::<Vec<_>>();
        self.roots = self.handles.insert_many(retained);
    }

    pub fn iter(&self) -> OrderedRootArenaIter<'_, T> {
        OrderedRootArenaIter {
            storage: &self.storage,
            handles: self.handles.span_or_empty(self.roots).iter(),
            marker: PhantomData,
        }
    }

    /// Select existing roots without moving or retiring their backing rows.
    /// Handles already published by the producer keep their exact payloads;
    /// only this view's ordered roster changes.
    pub fn retain(&mut self, mut keep: impl FnMut(&T) -> bool) {
        let retained = self
            .handles
            .span_or_empty(self.roots)
            .iter()
            .copied()
            .filter(|handle| keep(self.storage.get(*handle)))
            .collect::<Vec<_>>();
        if retained.len() != self.roots.len() {
            self.roots = self.handles.insert_many(retained);
        }
    }

    pub fn for_each_mut(&mut self, mut visit: impl FnMut(&mut T)) {
        let handles = self.handles.span_or_empty(self.roots);
        let storage = &mut self.storage;

        for handle in handles {
            visit(storage.get_mut(*handle));
        }
    }

    pub fn find_mut(&mut self, mut matches: impl FnMut(&T) -> bool) -> Option<&mut T> {
        let handles = self.handles.span_or_empty(self.roots);
        let storage = &mut self.storage;

        for handle in handles {
            if matches(storage.get(*handle)) {
                return Some(storage.get_mut(*handle));
            }
        }

        None
    }

    /// Combined freshness of backing payloads and the selected root roster.
    /// Same-length rewrites and membership changes invalidate consumers while
    /// retained declaration handles continue to name the same storage.
    #[inline]
    pub fn revision(&self) -> u64 {
        self.storage
            .revision()
            .wrapping_add(self.handles.revision())
    }
}

impl<T: Default> Default for OrderedRootArena<T> {
    fn default() -> Self {
        Self {
            handles: Arena::new(),
            roots: HandleSpan::empty(),
            storage: Arena::new(),
        }
    }
}

impl<T: Default> Index<usize> for OrderedRootArena<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        let handle = self.handles.span_or_empty(self.roots)[index];

        self.storage.get(handle)
    }
}

impl<T: Default> IndexMut<usize> for OrderedRootArena<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        let handle = self.handles.span_or_empty(self.roots)[index];

        self.storage.get_mut(handle)
    }
}

impl<'arena, T: Default> IntoIterator for &'arena OrderedRootArena<T> {
    type Item = &'arena T;
    type IntoIter = OrderedRootArenaIter<'arena, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

pub struct OrderedRootArenaIter<'arena, T: Default> {
    storage: &'arena Arena<T>,
    handles: std::slice::Iter<'arena, Handle<T>>,
    marker: PhantomData<&'arena T>,
}

impl<'arena, T: Default> Iterator for OrderedRootArenaIter<'arena, T> {
    type Item = &'arena T;

    fn next(&mut self) -> Option<Self::Item> {
        let handle = self.handles.next()?;

        Some(self.storage.get(*handle))
    }
}

#[cfg(test)]
mod tests {
    use super::OrderedRootArena;

    #[test]
    fn selecting_roots_preserves_retained_identity_and_changes_revision() {
        let mut roots = OrderedRootArena::<u64>::new();
        let first = roots.push(10);
        let removed = roots.push(20);
        let last = roots.push(30);
        let revision = roots.revision();
        roots.retain(|value| *value != 20);
        assert_eq!(roots.get(first), &10);
        assert_eq!(roots.get(last), &30);
        assert_eq!(roots[1], 30);
        assert!(!roots.contains(removed));
        assert!(roots.revision() > revision);
    }

    #[test]
    fn preserves_insertion_order_over_stable_handles() {
        let mut arena = OrderedRootArena::<String>::new();

        let first = arena.push("alpha".to_owned());
        let second = arena.push("beta".to_owned());

        assert_eq!(arena.len(), 2);
        assert_eq!(arena.first().map(String::as_str), Some("alpha"));
        assert_eq!(arena[0], "alpha");
        assert_eq!(arena[1], "beta");
        assert_ne!(first, second);
        assert_eq!(
            arena.iter().map(String::as_str).collect::<Vec<_>>(),
            ["alpha", "beta"]
        );
    }

    #[test]
    fn mutates_roots_in_order() {
        let mut arena = OrderedRootArena::<usize>::new();
        arena.push(1);
        arena.push(2);

        arena.for_each_mut(|value| *value *= 10);
        *arena.find_mut(|value| *value == 20).expect("root exists") = 25;

        assert_eq!(arena.iter().copied().collect::<Vec<_>>(), [10, 25]);
    }

    #[test]
    fn selecting_roots_preserves_backing_handles_and_moves_freshness() {
        let mut arena = OrderedRootArena::<usize>::new();
        let first = arena.push(1);
        let second = arena.push(2);
        let before = arena.revision();
        arena.retain(|value| *value == 2);
        assert_eq!(arena.iter().copied().collect::<Vec<_>>(), [2]);
        assert_eq!(*arena.storage.get(first), 1);
        assert_eq!(*arena.storage.get(second), 2);
        assert_ne!(before, arena.revision());
        let selected = arena.revision();
        arena.retain(|_| true);
        assert_eq!(selected, arena.revision());
        arena.push(3);
        assert_eq!(arena.iter().copied().collect::<Vec<_>>(), [2, 3]);
        assert_eq!(*arena.storage.get(second), 2);
    }
}
