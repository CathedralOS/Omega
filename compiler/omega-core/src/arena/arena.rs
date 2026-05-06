use std::marker::PhantomData;

use crate::arena::{Handle, HandleSpan};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arena<T> {
    items: Vec<T>,
    generations: Vec<u32>,
    occupied: Vec<bool>,
    free_indices: Vec<u32>,
    active_count: usize,
}

impl<T: Default> Arena<T> {
    pub fn new() -> Self {
        Self {
            items: vec![T::default()],
            generations: vec![0],
            occupied: vec![true],
            free_indices: Vec::new(),
            active_count: 0,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        let capacity_with_dummy = capacity.checked_add(1).expect("arena capacity overflow");
        let mut items = Vec::with_capacity(capacity_with_dummy);
        let mut generations = Vec::with_capacity(capacity_with_dummy);
        let mut occupied = Vec::with_capacity(capacity_with_dummy);

        items.push(T::default());
        generations.push(0);
        occupied.push(true);

        Self {
            items,
            generations,
            occupied,
            free_indices: Vec::new(),
            active_count: 0,
        }
    }

    pub fn insert(&mut self, item: T) -> Handle<T> {
        if let Some(arena_index) = self.free_indices.pop() {
            let index = usize::try_from(arena_index).expect("arena index overflow");

            self.items[index] = item;
            self.occupied[index] = true;
            self.active_count = self
                .active_count
                .checked_add(1)
                .expect("arena active count overflow");

            return Handle::from_parts(arena_index, self.generations[index]);
        }

        let arena_index = self.items.len().try_into().expect("arena index overflow");

        self.items.push(item);
        self.generations.push(1);
        self.occupied.push(true);
        self.active_count = self
            .active_count
            .checked_add(1)
            .expect("arena active count overflow");

        Handle::from_arena_index(arena_index)
    }

    pub fn insert_many(&mut self, items: impl IntoIterator<Item = T>) -> HandleSpan<T> {
        // Spans promise contiguous storage, so bulk insert appends instead of
        // consuming arbitrary free-list slots.
        let start_index = self.items.len().try_into().expect("arena index overflow");
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

    pub fn get(&self, handle: Handle<T>) -> &T {
        let index = self.index_from_valid_handle(handle);

        self.items.get(index).unwrap_or_else(|| self.dummy())
    }

    pub fn get_mut(&mut self, handle: Handle<T>) -> &mut T {
        let index = self.index_from_valid_handle(handle);

        if index < self.items.len() {
            &mut self.items[index]
        } else {
            &mut self.items[0]
        }
    }

    pub fn free(&mut self, handle: Handle<T>) -> bool {
        let index = self.index_from_valid_handle(handle);

        if index == 0 {
            return false;
        }

        self.items[index] = T::default();
        self.occupied[index] = false;
        self.generations[index] = next_generation(self.generations[index]);
        self.free_indices
            .push(u32::try_from(index).expect("arena index overflow"));
        self.active_count -= 1;

        true
    }

    pub fn is_valid(&self, handle: Handle<T>) -> bool {
        self.index_from_valid_handle(handle) != 0
    }

    pub fn span(&self, span: HandleSpan<T>) -> Option<&[T]> {
        if span.is_empty() {
            return Some(&[]);
        }

        let start = self.index_from_valid_handle(span.start());
        if start == 0 {
            return None;
        }

        let count = usize::try_from(span.count()).ok()?;
        let end = start.checked_add(count)?;
        let occupied = self.occupied.get(start..end)?;

        if occupied.iter().any(|occupied| !occupied) {
            return None;
        }

        self.items.get(start..end)
    }

    pub fn span_mut(&mut self, span: HandleSpan<T>) -> Option<&mut [T]> {
        if span.is_empty() {
            return Some(&mut []);
        }

        let start = self.index_from_valid_handle(span.start());
        if start == 0 {
            return None;
        }

        let count = usize::try_from(span.count()).ok()?;
        let end = start.checked_add(count)?;
        let occupied = self.occupied.get(start..end)?;

        if occupied.iter().any(|occupied| !occupied) {
            return None;
        }

        self.items.get_mut(start..end)
    }

    pub fn len(&self) -> usize {
        self.active_count
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn clear(&mut self) {
        for index in 1..self.items.len() {
            self.items[index] = T::default();
            self.generations[index] = next_generation(self.generations[index]);
            self.occupied[index] = false;
        }

        self.free_indices.clear();
        self.free_indices.extend(
            (1..self.items.len()).map(|index| u32::try_from(index).expect("arena index overflow")),
        );
        self.active_count = 0;
    }

    pub fn storage_slice(&self) -> &[T] {
        &self.items[1..]
    }

    pub fn dummy(&self) -> &T {
        &self.items[0]
    }

    pub fn iter(&self) -> ArenaIter<'_, T> {
        ArenaIter {
            arena: self,
            index: 1,
            marker: PhantomData,
        }
    }

    fn index_from_valid_handle(&self, handle: Handle<T>) -> usize {
        if !handle.is_valid() || handle.generation() == 0 {
            return 0;
        }

        let index = usize::try_from(handle.arena_index()).unwrap_or(0);

        if self
            .generations
            .get(index)
            .is_some_and(|generation| *generation == handle.generation())
            && self.occupied.get(index).is_some_and(|occupied| *occupied)
        {
            index
        } else {
            0
        }
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

            let arena_index = u32::try_from(index).expect("arena index overflow");
            let handle = Handle::from_parts(arena_index, self.arena.generations[index]);

            return Some((handle, &self.arena.items[index]));
        }

        None
    }
}

fn next_generation(generation: u32) -> u32 {
    let next = generation.wrapping_add(1);

    if next == 0 { 1 } else { next }
}
