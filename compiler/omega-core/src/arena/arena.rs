use std::marker::PhantomData;

use crate::arena::{Handle, HandleSpan};

pub struct Arena<T> {
    items: Vec<T>,
}

impl<T: Default> Arena<T> {
    pub fn new() -> Self {
        Self {
            items: vec![T::default()],
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        let mut items =
            Vec::with_capacity(capacity.checked_add(1).expect("arena capacity overflow"));
        items.push(T::default());

        Self { items }
    }

    pub fn insert(&mut self, item: T) -> Handle<T> {
        let arena_index = self.items.len().try_into().expect("arena index overflow");

        self.items.push(item);

        Handle::from_arena_index(arena_index)
    }

    pub fn insert_many(&mut self, items: impl IntoIterator<Item = T>) -> HandleSpan<T> {
        let start_index = self.items.len().try_into().expect("arena index overflow");
        let mut count = 0u32;

        for item in items {
            self.items.push(item);
            count = count.checked_add(1).expect("arena span count overflow");
        }

        if count == 0 {
            HandleSpan::empty()
        } else {
            HandleSpan::from_parts(Handle::from_arena_index(start_index), count)
        }
    }

    pub fn get(&self, handle: Handle<T>) -> &T {
        self.items
            .get(self.index_from_handle(handle))
            .unwrap_or_else(|| self.dummy())
    }

    pub fn get_mut(&mut self, handle: Handle<T>) -> &mut T {
        let index = self.index_from_handle(handle);

        if index < self.items.len() {
            &mut self.items[index]
        } else {
            &mut self.items[0]
        }
    }

    pub fn span(&self, span: HandleSpan<T>) -> Option<&[T]> {
        if span.is_empty() {
            return Some(&[]);
        }

        let start = self.index_from_handle(span.start());
        let count = usize::try_from(span.count()).ok()?;
        let end = start.checked_add(count)?;

        self.items.get(start..end)
    }

    pub fn span_mut(&mut self, span: HandleSpan<T>) -> Option<&mut [T]> {
        if span.is_empty() {
            return Some(&mut []);
        }

        let start = self.index_from_handle(span.start());
        let count = usize::try_from(span.count()).ok()?;
        let end = start.checked_add(count)?;

        self.items.get_mut(start..end)
    }

    pub fn len(&self) -> usize {
        self.items.len() - 1
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.items.push(T::default());
    }

    pub fn as_slice(&self) -> &[T] {
        &self.items[1..]
    }

    pub fn dummy(&self) -> &T {
        &self.items[0]
    }

    pub fn iter(&self) -> ArenaIter<'_, T> {
        ArenaIter {
            inner: self.items[1..].iter().enumerate(),
            marker: PhantomData,
        }
    }

    fn index_from_handle(&self, handle: Handle<T>) -> usize {
        if !handle.is_valid() || handle.generation() == 0 {
            return 0;
        }

        usize::try_from(handle.arena_index()).unwrap_or(0)
    }
}

impl<T: Default> Default for Arena<T> {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ArenaIter<'arena, T> {
    inner: std::iter::Enumerate<std::slice::Iter<'arena, T>>,
    marker: PhantomData<&'arena T>,
}

impl<'arena, T> Iterator for ArenaIter<'arena, T> {
    type Item = (Handle<T>, &'arena T);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|(index, item)| {
            let arena_index = u32::try_from(index + 1).expect("arena index overflow");

            (Handle::from_arena_index(arena_index), item)
        })
    }
}
