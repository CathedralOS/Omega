//! Start here. The contiguous `Arena`: one per node kind, addressed by
//! `Handle` and iterated in insertion order.

use std::marker::PhantomData;
use std::ops::Range;

use crate::{Handle, HandleSpan};

#[derive(Debug)]
pub struct Arena<T> {
    dummy: T,
    items: Vec<T>,
    generations: Vec<u32>,
    occupied: Vec<bool>,
    free_indices: Vec<u32>,
    active_count: usize,
    /// No slot has been freed or cleared since the arena was last empty, so
    /// every slot is occupied at generation 1. A span whose start handle
    /// resolves then resolves whole within bounds, without revisiting each
    /// row's occupancy and generation. Derived from the metadata above, so it
    /// takes no part in equality.
    fresh: bool,
}

impl<T: PartialEq> PartialEq for Arena<T> {
    fn eq(&self, other: &Self) -> bool {
        self.dummy == other.dummy
            && self.items == other.items
            && self.generations == other.generations
            && self.occupied == other.occupied
            && self.free_indices == other.free_indices
            && self.active_count == other.active_count
    }
}

impl<T: Eq> Eq for Arena<T> {}

impl<T: Clone> Clone for Arena<T> {
    fn clone(&self) -> Self {
        Self {
            dummy: self.dummy.clone(),
            items: self.items.clone(),
            generations: self.generations.clone(),
            occupied: self.occupied.clone(),
            free_indices: self.free_indices.clone(),
            active_count: self.active_count,
            fresh: self.fresh,
        }
    }

    fn clone_from(&mut self, source: &Self) {
        self.dummy.clone_from(&source.dummy);
        self.generations.clone_from(&source.generations);
        self.occupied.clone_from(&source.occupied);
        self.free_indices.clone_from(&source.free_indices);
        self.active_count = source.active_count;
        self.fresh = source.fresh;

        // Vec clones may unwind after truncating or partially extending the
        // payload. Keep the metadata for exactly that initialized prefix; a
        // failed clone need not preserve values, but the arena must stay usable.
        struct PayloadCloneGuard<'arena, T> {
            arena: &'arena mut Arena<T>,
            complete: bool,
        }

        impl<T> Drop for PayloadCloneGuard<'_, T> {
            fn drop(&mut self) {
                if self.complete {
                    return;
                }
                let length = self.arena.items.len();
                self.arena.generations.truncate(length);
                self.arena.occupied.truncate(length);
                self.arena
                    .free_indices
                    .retain(|arena_index| storage_index_from_arena_index(*arena_index) < length);
                self.arena.active_count = self
                    .arena
                    .occupied
                    .iter()
                    .filter(|occupied| **occupied)
                    .count();
            }
        }

        let mut guard = PayloadCloneGuard {
            arena: self,
            complete: false,
        };
        guard.arena.items.clone_from(&source.items);
        guard.complete = true;
    }
}

pub struct ArenaSpanInserter<'arena, T> {
    arena: &'arena mut Arena<T>,
    count: u32,
}

impl<T: Default> Arena<T> {
    pub fn new() -> Self {
        Self {
            dummy: T::default(),
            items: Vec::new(),
            generations: Vec::new(),
            occupied: Vec::new(),
            free_indices: Vec::new(),
            active_count: 0,
            fresh: true,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            dummy: T::default(),
            items: Vec::with_capacity(capacity),
            generations: Vec::with_capacity(capacity),
            occupied: Vec::with_capacity(capacity),
            free_indices: Vec::new(),
            active_count: 0,
            fresh: true,
        }
    }

    pub fn reserve(&mut self, additional: usize) {
        self.items.reserve(additional);
        self.generations.reserve(additional);
        self.occupied.reserve(additional);
    }

    pub fn insert(&mut self, item: T) -> Handle<T> {
        if let Some(arena_index) = self.free_indices.pop() {
            let index = storage_index_from_arena_index(arena_index);

            self.items[index] = item;
            self.occupied[index] = true;
            self.active_count = self
                .active_count
                .checked_add(1)
                .expect("arena active count overflow");

            return Handle::from_parts(arena_index, self.generations[index]);
        }

        let arena_index = next_arena_index(self.items.len());

        self.items.push(item);
        self.generations.push(1);
        self.occupied.push(true);
        self.active_count = self
            .active_count
            .checked_add(1)
            .expect("arena active count overflow");

        Handle::from_arena_index(arena_index)
    }

    pub fn append(&mut self, item: T) -> Handle<T> {
        let arena_index = next_arena_index(self.items.len());

        self.items.push(item);
        self.generations.push(1);
        self.occupied.push(true);
        self.active_count = self
            .active_count
            .checked_add(1)
            .expect("arena active count overflow");

        Handle::from_arena_index(arena_index)
    }

    /// A span grows in place only while its rows still end at the arena tail
    /// carrying generation 1, the generation every fresh append receives. A
    /// span that went stale — other rows landed on this arena's tail between
    /// appends, or the span was built on recycled-generation slots — is first
    /// relocated to the tail, the same snapshot move `copy_span_pair` performs
    /// for retained prefixes, so the span keeps resolving to the same row
    /// sequence. A stale span whose rows no longer resolve (freed or
    /// generation-shifted slots) has nothing to relocate and still panics.
    pub fn append_to_span(&mut self, span: &mut HandleSpan<T>, item: T) -> Handle<T>
    where
        T: Clone,
    {
        if !span.is_empty()
            && (span.start().generation() != 1
                || span
                    .start()
                    .arena_index()
                    .checked_add(span.count())
                    .expect("arena span index overflow")
                    != next_arena_index(self.items.len()))
        {
            assert!(
                self.valid_span_range(*span).is_some(),
                "arena span append on an unresolvable span"
            );
            *span = self.copy_span_pair(*span, HandleSpan::empty());
        }

        let handle = self.append(item);
        *span = if span.is_empty() {
            HandleSpan::from_parts(handle, 1)
        } else {
            HandleSpan::from_parts(
                span.start(),
                span.count()
                    .checked_add(1)
                    .expect("arena span count overflow"),
            )
        };
        handle
    }

    pub fn pop_last_appended(&mut self, handle: Handle<T>) -> Option<T> {
        let index = self.index_from_valid_handle(handle);
        if index == invalid_index() || index.checked_add(1)? != self.items.len() {
            return None;
        }
        if self.generations.get(index).copied()? != handle.generation()
            || !self.occupied.get(index).copied()?
        {
            return None;
        }

        self.generations.pop();
        self.occupied.pop();
        self.active_count = self.active_count.checked_sub(1)?;

        self.items.pop()
    }

    pub fn insert_many(&mut self, items: impl IntoIterator<Item = T>) -> HandleSpan<T> {
        // Spans promise contiguous storage, so bulk insert appends instead of
        // consuming arbitrary free-list slots.
        let start_index = next_arena_index(self.items.len());
        let mut count = 0u32;

        for item in items {
            self.items.push(item);
            self.generations.push(1);
            self.occupied.push(true);
            count = count.checked_add(1).expect("arena span count overflow");
        }

        if count == 0 {
            HandleSpan::empty()
        } else {
            self.active_count = self
                .active_count
                .checked_add(usize::try_from(count).expect("arena span count overflow"))
                .expect("arena active count overflow");

            HandleSpan::from_parts(Handle::from_arena_index(start_index), count)
        }
    }

    pub fn insert_many_with(
        &mut self,
        insert_items: impl FnOnce(&mut ArenaSpanInserter<'_, T>),
    ) -> HandleSpan<T> {
        // Spans promise contiguous storage. This variant lets recursive
        // producers emit directly into the arena without staging a Vec.
        let start_index = next_arena_index(self.items.len());
        let mut inserter = ArenaSpanInserter {
            arena: self,
            count: 0,
        };

        insert_items(&mut inserter);

        if inserter.count == 0 {
            HandleSpan::empty()
        } else {
            HandleSpan::from_parts(Handle::from_arena_index(start_index), inserter.count)
        }
    }

    pub fn try_insert_many_with<E>(
        &mut self,
        insert_items: impl FnOnce(&mut ArenaSpanInserter<'_, T>) -> Result<(), E>,
    ) -> Result<HandleSpan<T>, E> {
        // Spans promise contiguous storage. This fallible variant lets
        // producers emit directly into the arena while preserving rollback.
        let start_len = self.items.len();
        let start_active_count = self.active_count;
        let start_index = next_arena_index(start_len);
        let mut inserter = ArenaSpanInserter {
            arena: self,
            count: 0,
        };

        if let Err(error) = insert_items(&mut inserter) {
            inserter.arena.items.truncate(start_len);
            inserter.arena.generations.truncate(start_len);
            inserter.arena.occupied.truncate(start_len);
            inserter.arena.active_count = start_active_count;
            return Err(error);
        }

        if inserter.count == 0 {
            Ok(HandleSpan::empty())
        } else {
            Ok(HandleSpan::from_parts(
                Handle::from_arena_index(start_index),
                inserter.count,
            ))
        }
    }

    pub fn try_insert_many<E>(
        &mut self,
        items: impl IntoIterator<Item = Result<T, E>>,
    ) -> Result<HandleSpan<T>, E> {
        // Spans promise contiguous storage, so bulk insert appends instead of
        // consuming arbitrary free-list slots. On failure, discard the partial
        // append so callers do not leave unreachable arena payloads behind.
        let start_len = self.items.len();
        let start_index = next_arena_index(start_len);
        let mut count = 0u32;

        for item in items {
            match item {
                Ok(item) => {
                    self.items.push(item);
                    self.generations.push(1);
                    self.occupied.push(true);
                    count = count.checked_add(1).expect("arena span count overflow");
                }
                Err(error) => {
                    self.items.truncate(start_len);
                    self.generations.truncate(start_len);
                    self.occupied.truncate(start_len);
                    return Err(error);
                }
            }
        }

        if count == 0 {
            Ok(HandleSpan::empty())
        } else {
            self.active_count = self
                .active_count
                .checked_add(usize::try_from(count).expect("arena span count overflow"))
                .expect("arena active count overflow");

            Ok(HandleSpan::from_parts(
                Handle::from_arena_index(start_index),
                count,
            ))
        }
    }

    pub fn copy_span_pair(&mut self, first: HandleSpan<T>, second: HandleSpan<T>) -> HandleSpan<T>
    where
        T: Clone,
    {
        let first_range = self.valid_span_range(first);
        let second_range = self.valid_span_range(second);
        let count = first_range
            .as_ref()
            .map(|range| range.len())
            .unwrap_or(0)
            .checked_add(second_range.as_ref().map(|range| range.len()).unwrap_or(0))
            .expect("arena copied span count overflow");

        if count == 0 {
            return HandleSpan::empty();
        }

        let start_index = next_arena_index(self.items.len());

        if let Some(first_range) = first_range {
            for index in first_range {
                self.items.push(self.items[index].clone());
                self.generations.push(1);
                self.occupied.push(true);
            }
        }

        if let Some(second_range) = second_range {
            for index in second_range {
                self.items.push(self.items[index].clone());
                self.generations.push(1);
                self.occupied.push(true);
            }
        }

        self.active_count = self
            .active_count
            .checked_add(count)
            .expect("arena active count overflow");

        HandleSpan::from_parts(
            Handle::from_arena_index(start_index),
            count.try_into().expect("arena span count overflow"),
        )
    }

    pub fn get(&self, handle: Handle<T>) -> &T {
        let index = self.index_from_valid_handle(handle);

        self.items.get(index).unwrap_or_else(|| self.dummy())
    }

    pub fn get_mut(&mut self, handle: Handle<T>) -> &mut T {
        let index = self.index_from_valid_handle(handle);

        if index < self.items.len() {
            &mut self.items[index]
        } else {
            &mut self.dummy
        }
    }

    pub fn free(&mut self, handle: Handle<T>) -> bool {
        let index = self.index_from_valid_handle(handle);

        if index == invalid_index() {
            return false;
        }

        self.items[index] = T::default();
        self.occupied[index] = false;
        self.generations[index] = next_generation(self.generations[index]);
        self.free_indices.push(next_arena_index(index));
        self.active_count -= 1;
        self.fresh = false;

        true
    }

    pub fn is_valid(&self, handle: Handle<T>) -> bool {
        self.index_from_valid_handle(handle) != invalid_index()
    }

    pub fn span(&self, span: HandleSpan<T>) -> Option<&[T]> {
        if span.is_empty() {
            return Some(&[]);
        }

        let range = self.valid_span_range(span)?;

        self.items.get(range)
    }

    pub fn span_or_empty(&self, span: HandleSpan<T>) -> &[T] {
        self.span(span).unwrap_or(&self.items[0..0])
    }

    pub fn span_mut(&mut self, span: HandleSpan<T>) -> Option<&mut [T]> {
        if span.is_empty() {
            return Some(&mut []);
        }

        let range = self.valid_span_range(span)?;

        self.items.get_mut(range)
    }

    pub fn span_mut_or_empty(&mut self, span: HandleSpan<T>) -> &mut [T] {
        if let Some(range) = self.valid_span_range(span) {
            &mut self.items[range]
        } else {
            &mut self.items[0..0]
        }
    }

    fn valid_span_range(&self, span: HandleSpan<T>) -> Option<Range<usize>> {
        let start = self.index_from_valid_handle(span.start());
        if start == invalid_index() {
            return None;
        }

        let count = usize::try_from(span.count()).ok()?;
        let end = start.checked_add(count)?;
        if self.fresh {
            return (end <= self.items.len()).then_some(start..end);
        }
        let occupied = self.occupied.get(start..end)?;
        let generations = self.generations.get(start..end)?;

        if occupied.iter().any(|occupied| !occupied) {
            return None;
        }

        if generations
            .iter()
            .any(|generation| *generation != span.start().generation())
        {
            return None;
        }

        Some(start..end)
    }

    pub fn len(&self) -> usize {
        self.active_count
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn clear(&mut self) {
        for index in 0..self.items.len() {
            self.items[index] = T::default();
            self.generations[index] = next_generation(self.generations[index]);
            self.occupied[index] = false;
        }

        self.free_indices.clear();
        self.free_indices
            .extend((0..self.items.len()).map(next_arena_index));
        self.active_count = 0;
        self.fresh = self.items.is_empty();
    }

    pub fn reset_retain_capacity(&mut self) {
        self.items.clear();
        self.generations.clear();
        self.occupied.clear();
        self.free_indices.clear();
        self.active_count = 0;
        self.fresh = true;
    }

    pub fn map<U: Default>(self, mut map_item: impl FnMut(T) -> U) -> Arena<U> {
        let mut items = Vec::with_capacity(self.items.len());
        items.extend(self.items.into_iter().map(&mut map_item));

        Arena {
            dummy: U::default(),
            items,
            generations: self.generations,
            occupied: self.occupied,
            free_indices: self.free_indices,
            active_count: self.active_count,
            fresh: self.fresh,
        }
    }

    pub fn storage_slice(&self) -> &[T] {
        &self.items
    }

    pub fn dummy(&self) -> &T {
        &self.dummy
    }

    pub fn iter(&self) -> ArenaIter<'_, T> {
        ArenaIter {
            arena: self,
            index: 0,
            marker: PhantomData,
        }
    }

    pub fn into_items(self) -> impl Iterator<Item = T> {
        self.items
            .into_iter()
            .zip(self.occupied)
            .filter_map(|(item, occupied)| occupied.then_some(item))
    }

    pub fn into_span_items(self, span: HandleSpan<T>) -> impl Iterator<Item = T> {
        let range = self.valid_span_range(span).unwrap_or(0..0);

        self.items
            .into_iter()
            .enumerate()
            .filter_map(move |(index, item)| range.contains(&index).then_some(item))
    }

    pub fn for_each_mut(&mut self, mut visit: impl FnMut(Handle<T>, &mut T)) {
        for index in 0..self.items.len() {
            if !self.occupied[index] {
                continue;
            }

            let arena_index = next_arena_index(index);
            let handle = Handle::from_parts(arena_index, self.generations[index]);

            visit(handle, &mut self.items[index]);
        }
    }

    fn index_from_valid_handle(&self, handle: Handle<T>) -> usize {
        if !handle.is_valid() || handle.generation() == 0 {
            return invalid_index();
        }

        let index = storage_index_from_arena_index(handle.arena_index());

        if self
            .generations
            .get(index)
            .is_some_and(|generation| *generation == handle.generation())
            && self.occupied.get(index).is_some_and(|occupied| *occupied)
        {
            index
        } else {
            invalid_index()
        }
    }
}

impl<T: Default> ArenaSpanInserter<'_, T> {
    pub fn insert(&mut self, item: T) -> Handle<T> {
        let arena_index = next_arena_index(self.arena.items.len());

        self.arena.items.push(item);
        self.arena.generations.push(1);
        self.arena.occupied.push(true);
        self.arena.active_count = self
            .arena
            .active_count
            .checked_add(1)
            .expect("arena active count overflow");
        self.count = self
            .count
            .checked_add(1)
            .expect("arena span count overflow");

        Handle::from_arena_index(arena_index)
    }
}

impl<T: Default> Default for Arena<T> {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ArenaIter<'arena, T> {
    arena: &'arena Arena<T>,
    index: usize,
    marker: PhantomData<&'arena T>,
}

impl<'arena, T: Default> Iterator for ArenaIter<'arena, T> {
    type Item = (Handle<T>, &'arena T);

    fn next(&mut self) -> Option<Self::Item> {
        while self.index < self.arena.items.len() {
            let index = self.index;
            self.index += 1;

            if !self.arena.occupied[index] {
                continue;
            }

            let arena_index = next_arena_index(index);
            let handle = Handle::from_parts(arena_index, self.arena.generations[index]);

            return Some((handle, &self.arena.items[index]));
        }

        None
    }
}

fn invalid_index() -> usize {
    usize::MAX
}

fn next_arena_index(storage_len: usize) -> u32 {
    storage_len
        .checked_add(1)
        .and_then(|index| index.try_into().ok())
        .expect("arena index overflow")
}

fn storage_index_from_arena_index(arena_index: u32) -> usize {
    arena_index
        .checked_sub(1)
        .and_then(|index| usize::try_from(index).ok())
        .unwrap_or_else(invalid_index)
}

fn next_generation(generation: u32) -> u32 {
    let next = generation.wrapping_add(1);

    if next == 0 { 1 } else { next }
}

#[cfg(test)]
mod tests;
