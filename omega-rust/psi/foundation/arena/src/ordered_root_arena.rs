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

    pub fn iter(&self) -> OrderedRootArenaIter<'_, T> {
        OrderedRootArenaIter {
            storage: &self.storage,
            handles: self.handles.span_or_empty(self.roots).iter(),
            marker: PhantomData,
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
}
