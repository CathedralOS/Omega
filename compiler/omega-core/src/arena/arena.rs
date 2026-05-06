use std::marker::PhantomData;

use crate::arena::{Handle, HandleSpan};

pub struct Arena<T> {
    items: Vec<T>,
}

impl<T> Arena<T> {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
        }
    }

    pub fn insert(&mut self, item: T) -> Handle<T> {
        let arena_index = self
            .items
            .len()
            .checked_add(1)
            .and_then(|index| u32::try_from(index).ok())
            .expect("arena index overflow");

        self.items.push(item);

        Handle::from_arena_index(arena_index)
    }

    pub fn insert_many(&mut self, items: impl IntoIterator<Item = T>) -> HandleSpan<T> {
        let start_index = self
            .items
            .len()
            .checked_add(1)
            .and_then(|index| u32::try_from(index).ok())
            .expect("arena index overflow");
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

    pub fn get(&self, handle: Handle<T>) -> Option<&T> {
        self.index_from_handle(handle)
            .and_then(|index| self.items.get(index))
    }

    pub fn get_mut(&mut self, handle: Handle<T>) -> Option<&mut T> {
        self.index_from_handle(handle)
            .and_then(|index| self.items.get_mut(index))
    }

    pub fn span(&self, span: HandleSpan<T>) -> Option<&[T]> {
        let start = self.index_from_handle(span.start())?;
        let count = usize::try_from(span.count()).ok()?;
        let end = start.checked_add(count)?;

        self.items.get(start..end)
    }

    pub fn span_mut(&mut self, span: HandleSpan<T>) -> Option<&mut [T]> {
        let start = self.index_from_handle(span.start())?;
        let count = usize::try_from(span.count()).ok()?;
        let end = start.checked_add(count)?;

        self.items.get_mut(start..end)
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn as_slice(&self) -> &[T] {
        &self.items
    }

    pub fn iter(&self) -> ArenaIter<'_, T> {
        ArenaIter {
            inner: self.items.iter().enumerate(),
            marker: PhantomData,
        }
    }

    fn index_from_handle(&self, handle: Handle<T>) -> Option<usize> {
        if !handle.is_valid() {
            return None;
        }

        usize::try_from(handle.arena_index() - 1).ok()
    }
}

impl<T> Default for Arena<T> {
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
