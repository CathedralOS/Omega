use std::marker::PhantomData;
use std::ops::Range;

use crate::arena::{Handle, HandleSpan};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arena<T> {
    items: Vec<T>,
    generations: Vec<u32>,
    occupied: Vec<bool>,
    free_indices: Vec<u32>,
    active_count: usize,
}

pub struct ArenaSpanInserter<'arena, T> {
    arena: &'arena mut Arena<T>,
    count: u32,
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

    pub fn reserve(&mut self, additional: usize) {
        self.items.reserve(additional);
        self.generations.reserve(additional);
        self.occupied.reserve(additional);
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

    pub fn append(&mut self, item: T) -> Handle<T> {
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

    pub fn append_to_span(&mut self, span: &mut HandleSpan<T>, item: T) -> Handle<T> {
        let next_index: u32 = self.items.len().try_into().expect("arena index overflow");
        if !span.is_empty() {
            let expected_index = span
                .start()
                .arena_index()
                .checked_add(span.count())
                .expect("arena span index overflow");
            assert_eq!(
                next_index, expected_index,
                "arena span append must be contiguous"
            );
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
        let index = usize::try_from(handle.arena_index()).ok()?;
        if index == 0 || index.checked_add(1)? != self.items.len() {
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

    pub fn insert_many_with(
        &mut self,
        insert_items: impl FnOnce(&mut ArenaSpanInserter<'_, T>),
    ) -> HandleSpan<T> {
        // Spans promise contiguous storage. This variant lets recursive
        // producers emit directly into the arena without staging a Vec.
        let start_index = self.items.len().try_into().expect("arena index overflow");
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
        let start_index = start_len.try_into().expect("arena index overflow");
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
        let start_index = start_len.try_into().expect("arena index overflow");
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

        let start_index = self.items.len().try_into().expect("arena index overflow");

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
        if start == 0 {
            return None;
        }

        let count = usize::try_from(span.count()).ok()?;
        let end = start.checked_add(count)?;
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

    pub fn reset_retain_capacity(&mut self) {
        self.items.truncate(1);
        self.generations.truncate(1);
        self.occupied.truncate(1);
        self.free_indices.clear();
        self.active_count = 0;
    }

    pub fn map<U: Default>(self, mut map_item: impl FnMut(T) -> U) -> Arena<U> {
        let mut items = Vec::with_capacity(self.items.len());
        let mut source_items = self.items.into_iter();

        if source_items.next().is_some() {
            items.push(U::default());
        }
        items.extend(source_items.map(&mut map_item));

        Arena {
            items,
            generations: self.generations,
            occupied: self.occupied,
            free_indices: self.free_indices,
            active_count: self.active_count,
        }
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

    pub fn into_items(self) -> impl Iterator<Item = T> {
        self.items
            .into_iter()
            .zip(self.occupied)
            .enumerate()
            .filter_map(|(index, (item, occupied))| (index != 0 && occupied).then_some(item))
    }

    pub fn into_span_items(self, span: HandleSpan<T>) -> impl Iterator<Item = T> {
        let range = self.valid_span_range(span).unwrap_or(1..1);

        self.items
            .into_iter()
            .enumerate()
            .filter_map(move |(index, item)| range.contains(&index).then_some(item))
    }

    pub fn for_each_mut(&mut self, mut visit: impl FnMut(Handle<T>, &mut T)) {
        for index in 1..self.items.len() {
            if !self.occupied[index] {
                continue;
            }

            let arena_index = u32::try_from(index).expect("arena index overflow");
            let handle = Handle::from_parts(arena_index, self.generations[index]);

            visit(handle, &mut self.items[index]);
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

impl<T: Default> ArenaSpanInserter<'_, T> {
    pub fn insert(&mut self, item: T) -> Handle<T> {
        let arena_index = self
            .arena
            .items
            .len()
            .try_into()
            .expect("arena index overflow");

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

#[cfg(test)]
mod tests {
    use crate::arena::{Arena, Handle};

    #[test]
    fn resolves_zero_to_dummy_slot() {
        let mut arena = Arena::new();
        let invalid = Handle::<String>::invalid();
        let first = arena.insert("alpha".to_owned());
        let second = arena.insert("beta".to_owned());

        assert!(!invalid.is_valid());
        assert_eq!(arena.len(), 2);
        assert_eq!(first.arena_index(), 1);
        assert_eq!(second.arena_index(), 2);
        assert_eq!(arena.get(invalid).as_str(), "");
        assert_eq!(arena.get(first).as_str(), "alpha");
        assert_eq!(arena.get(second).as_str(), "beta");
    }

    #[test]
    fn invalidates_freed_handles() {
        let mut arena = Arena::new();
        let first = arena.insert("alpha".to_owned());

        assert_eq!(arena.get(first).as_str(), "alpha");
        assert!(arena.is_valid(first));
        assert!(arena.free(first));
        assert!(!arena.is_valid(first));
        assert_eq!(arena.get(first).as_str(), "");

        let reused = arena.insert("beta".to_owned());

        assert_eq!(reused.arena_index(), first.arena_index());
        assert_ne!(reused.generation(), first.generation());
        assert_eq!(arena.get(first).as_str(), "");
        assert_eq!(arena.get(reused).as_str(), "beta");
    }

    #[test]
    fn stores_contiguous_handle_spans() {
        let mut arena = Arena::new();
        let span = arena.insert_many(["alpha".to_owned(), "beta".to_owned(), "gamma".to_owned()]);

        assert_eq!(span.start().arena_index(), 1);
        assert_eq!(span.count(), 3);
        assert_eq!(
            arena.span(span).expect("span should resolve"),
            &["alpha".to_owned(), "beta".to_owned(), "gamma".to_owned()]
        );

        arena.span_mut(span).expect("span should resolve")[1] = "bravo".to_owned();

        assert_eq!(
            arena.span(span).expect("span should resolve"),
            &["alpha".to_owned(), "bravo".to_owned(), "gamma".to_owned()]
        );
    }

    #[test]
    fn appends_directly_to_contiguous_handle_spans() {
        let mut arena = Arena::new();
        let mut span = crate::arena::HandleSpan::empty();

        let alpha = arena.append_to_span(&mut span, "alpha".to_owned());
        let beta = arena.append_to_span(&mut span, "beta".to_owned());

        assert_eq!(alpha.arena_index(), 1);
        assert_eq!(beta.arena_index(), 2);
        assert_eq!(span.start(), alpha);
        assert_eq!(span.count(), 2);
        assert_eq!(
            arena.span(span).expect("span should resolve"),
            &["alpha".to_owned(), "beta".to_owned()]
        );
    }

    #[test]
    #[should_panic(expected = "arena span append must be contiguous")]
    fn panics_when_extending_stale_contiguous_spans() {
        let mut arena = Arena::new();
        let mut span = crate::arena::HandleSpan::empty();
        arena.append_to_span(&mut span, "alpha".to_owned());
        arena.append("gap".to_owned());

        arena.append_to_span(&mut span, "beta".to_owned());
    }

    #[test]
    fn fallible_span_insert_rolls_back_partial_appends() {
        let mut arena = Arena::new();
        let first = arena.insert("alpha".to_owned());

        let result =
            arena.try_insert_many([Ok("beta".to_owned()), Err("boom"), Ok("gamma".to_owned())]);

        assert_eq!(result, Err("boom"));
        assert_eq!(arena.len(), 1);
        assert_eq!(arena.get(first).as_str(), "alpha");

        let next = arena.append("delta".to_owned());

        assert_eq!(next.arena_index(), 2);
        assert_eq!(arena.get(next).as_str(), "delta");
    }

    #[test]
    fn fallible_span_insert_returns_contiguous_span_on_success() {
        let mut arena = Arena::new();
        let span = arena
            .try_insert_many::<&str>([Ok("alpha".to_owned()), Ok("beta".to_owned())])
            .expect("fallible insert should succeed");

        assert_eq!(span.start().arena_index(), 1);
        assert_eq!(span.count(), 2);
        assert_eq!(
            arena.span(span).expect("span should resolve"),
            &["alpha".to_owned(), "beta".to_owned()]
        );
    }

    #[test]
    fn fallible_direct_span_insert_rolls_back_partial_appends() {
        let mut arena = Arena::new();
        let first = arena.insert("alpha".to_owned());

        let result = arena.try_insert_many_with(|inserter| {
            inserter.insert("beta".to_owned());
            Err("boom")
        });

        assert_eq!(result, Err("boom"));
        assert_eq!(arena.len(), 1);
        assert_eq!(arena.get(first).as_str(), "alpha");

        let next = arena.append("delta".to_owned());

        assert_eq!(next.arena_index(), 2);
        assert_eq!(arena.get(next).as_str(), "delta");
    }

    #[test]
    fn fallible_direct_span_insert_returns_contiguous_span_on_success() {
        let mut arena = Arena::new();
        let span = arena
            .try_insert_many_with::<&str>(|inserter| {
                inserter.insert("alpha".to_owned());
                inserter.insert("beta".to_owned());
                Ok(())
            })
            .expect("fallible direct insert should succeed");

        assert_eq!(span.start().arena_index(), 1);
        assert_eq!(span.count(), 2);
        assert_eq!(
            arena.span(span).expect("span should resolve"),
            &["alpha".to_owned(), "beta".to_owned()]
        );
    }

    #[test]
    fn copies_two_spans_into_one_contiguous_span() {
        let mut arena = Arena::new();
        let first = arena.insert_many(["alpha".to_owned(), "beta".to_owned()]);
        let _gap = arena.insert_many(["gap".to_owned()]);
        let second = arena.insert_many(["gamma".to_owned(), "delta".to_owned()]);

        let copied = arena.copy_span_pair(first, second);

        assert_eq!(copied.start().arena_index(), 6);
        assert_eq!(copied.count(), 4);
        assert_eq!(
            arena.span(copied).expect("span should resolve"),
            &[
                "alpha".to_owned(),
                "beta".to_owned(),
                "gamma".to_owned(),
                "delta".to_owned()
            ]
        );
    }

    #[test]
    fn rejects_spans_with_freed_slots() {
        let mut arena = Arena::new();
        let span = arena.insert_many(["alpha".to_owned(), "beta".to_owned(), "gamma".to_owned()]);
        let middle = Handle::from_arena_index(2);

        assert!(arena.span(span).is_some());
        assert!(arena.free(middle));
        assert!(arena.span(span).is_none());
        assert!(arena.span_mut(span).is_none());
    }

    #[test]
    fn rejects_spans_with_reused_slots() {
        let mut arena = Arena::new();
        let span = arena.insert_many(["alpha".to_owned(), "beta".to_owned(), "gamma".to_owned()]);
        let middle = Handle::from_arena_index(2);

        assert!(arena.free(middle));
        let reused = arena.insert("delta".to_owned());

        assert_eq!(reused.arena_index(), middle.arena_index());
        assert!(arena.span(span).is_none());
        assert!(arena.span_mut(span).is_none());
    }

    #[test]
    fn clear_invalidates_existing_handles() {
        let mut arena = Arena::new();
        let first = arena.insert("alpha".to_owned());

        arena.clear();

        assert_eq!(arena.len(), 0);
        assert!(!arena.is_valid(first));
        assert_eq!(arena.get(first).as_str(), "");

        let reused = arena.insert("beta".to_owned());

        assert_eq!(reused.arena_index(), first.arena_index());
        assert_ne!(reused.generation(), first.generation());
        assert_eq!(arena.get(first).as_str(), "");
        assert_eq!(arena.get(reused).as_str(), "beta");
    }
}
