//! Start here. The contiguous `Arena`: one per node kind, addressed by
//! `Handle` and iterated in insertion order.
//!
//! An unretired page already has one generation and no holes. Keep that identity
//! in the arena instead of allocating and filling generation/occupancy vectors
//! beside every payload row. Retirement materializes those vectors in a cold
//! owner; payloads remain contiguous and every stale-handle check still applies.
//! The first retirement therefore initializes O(n) metadata. Append-only pages
//! avoid that work and the cold vector headers; reset and clone_from retain any
//! allocated buffers for subsequent reuse. Physical metadata mode is separate
//! from logical freshness so a failed payload operation keeps its prior contract.

use std::marker::PhantomData;
use std::ops::Range;

use crate::{Handle, HandleSpan};

#[derive(Default)]
struct SlotMetadata {
    generations: Vec<u32>,
    occupied: Vec<bool>,
    free_indices: Vec<u32>,
}

static EMPTY_SLOT_METADATA: SlotMetadata = SlotMetadata {
    generations: Vec::new(),
    occupied: Vec::new(),
    free_indices: Vec::new(),
};

impl Clone for SlotMetadata {
    fn clone(&self) -> Self {
        Self {
            generations: self.generations.clone(),
            occupied: self.occupied.clone(),
            free_indices: self.free_indices.clone(),
        }
    }
    fn clone_from(&mut self, source: &Self) {
        self.generations.clone_from(&source.generations);
        self.occupied.clone_from(&source.occupied);
        self.free_indices.clone_from(&source.free_indices);
    }
}

pub struct Arena<T> {
    dummy: T,
    items: Vec<T>,
    slot_metadata: Option<Box<SlotMetadata>>,
    active_count: usize,
    /// Generation assigned to every newly appended row. Ordinary arenas use
    /// one; worker-owned syntax pages use their page identity so handles remain
    /// globally distinct when the dense item vector publishes unchanged.
    fresh_generation: u32,
    /// No slot has been freed or cleared since the arena was last empty, so
    /// every slot is occupied at `fresh_generation`. A span whose start handle
    /// resolves then resolves whole within bounds, without revisiting each
    /// row's occupancy and generation. This logical freshness is independent of
    /// whether metadata storage has materialized: a payload operation may unwind
    /// after allocating metadata but before completing retirement.
    fresh: bool,
    // Physical storage mode is separate from freshness. Materializing rows must
    // not change the existing freshness contract if a payload operation unwinds.
    implicit_slot_metadata: bool,
    /// Advances on every call that can change the arena's contents — inserts,
    /// frees, truncations, wholesale replacement, and every mutable-slice or
    /// mutable-element escape. Caches that outlive one borrow key freshness on
    /// this counter instead of sampling lengths and boundary rows: a
    /// same-length in-place rewrite moves it exactly like an append does.
    /// Mutation metadata like `fresh`, so it takes no part in equality.
    revision: u64,
    rewrite_revision: u64,
}

impl<T> Arena<T> {
    #[inline]
    fn slot_metadata(&self) -> &SlotMetadata {
        self.slot_metadata
            .as_deref()
            .unwrap_or(&EMPTY_SLOT_METADATA)
    }

    fn slot_metadata_mut(&mut self) -> &mut SlotMetadata {
        self.slot_metadata
            .get_or_insert_with(|| Box::new(SlotMetadata::default()))
    }

    fn equal_slot_metadata(&self, other: &Self) -> bool {
        match (self.implicit_slot_metadata, other.implicit_slot_metadata) {
            (true, true) => true,
            (false, false) => {
                self.slot_metadata().generations == other.slot_metadata().generations
                    && self.slot_metadata().occupied == other.slot_metadata().occupied
            }
            (true, false) => {
                other
                    .slot_metadata()
                    .generations
                    .iter()
                    .all(|generation| *generation == self.fresh_generation)
                    && other
                        .slot_metadata()
                        .occupied
                        .iter()
                        .all(|occupied| *occupied)
            }
            (false, true) => {
                self.slot_metadata()
                    .generations
                    .iter()
                    .all(|generation| *generation == other.fresh_generation)
                    && self
                        .slot_metadata()
                        .occupied
                        .iter()
                        .all(|occupied| *occupied)
            }
        }
    }

    /// Compare allocation metadata and every stored row through a caller-owned
    /// equality operation. This includes vacant rows and the absent-value row,
    /// exactly like ordinary arena equality; revision caches remain excluded.
    pub fn equal_by(&self, other: &Self, mut equal: impl FnMut(&T, &T) -> bool) -> bool {
        equal(&self.dummy, &other.dummy)
            && self.items.len() == other.items.len()
            && self
                .items
                .iter()
                .zip(&other.items)
                .all(|(left, right)| equal(left, right))
            && self.equal_slot_metadata(other)
            && self.slot_metadata().free_indices == other.slot_metadata().free_indices
            && self.active_count == other.active_count
            && self.fresh_generation == other.fresh_generation
    }
}
impl<T: PartialEq> PartialEq for Arena<T> {
    fn eq(&self, other: &Self) -> bool {
        self.dummy == other.dummy
            && self.items == other.items
            && self.equal_slot_metadata(other)
            && self.slot_metadata().free_indices == other.slot_metadata().free_indices
            && self.active_count == other.active_count
            && self.fresh_generation == other.fresh_generation
    }
}

impl<T: Eq> Eq for Arena<T> {}

impl<T: std::fmt::Debug> std::fmt::Debug for Arena<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        struct UniformRows<T> {
            value: T,
            count: usize,
        }
        impl<T: std::fmt::Debug> std::fmt::Debug for UniformRows<T> {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter
                    .debug_list()
                    .entries(std::iter::repeat_n(&self.value, self.count))
                    .finish()
            }
        }
        let mut debug = formatter.debug_struct("Arena");
        debug
            .field("dummy", &self.dummy)
            .field("items", &self.items);
        if self.implicit_slot_metadata {
            debug.field(
                "generations",
                &UniformRows {
                    value: self.fresh_generation,
                    count: self.items.len(),
                },
            );
            debug.field(
                "occupied",
                &UniformRows {
                    value: true,
                    count: self.items.len(),
                },
            );
        } else {
            debug
                .field("generations", &self.slot_metadata().generations)
                .field("occupied", &self.slot_metadata().occupied);
        }
        debug
            .field("free_indices", &self.slot_metadata().free_indices)
            .field("active_count", &self.active_count)
            .field("fresh_generation", &self.fresh_generation)
            .field("fresh", &self.fresh)
            .field("revision", &self.revision)
            .finish()
    }
}

impl<T: Clone> Clone for Arena<T> {
    fn clone(&self) -> Self {
        Self {
            dummy: self.dummy.clone(),
            items: self.items.clone(),
            slot_metadata: if self.implicit_slot_metadata {
                None
            } else {
                self.slot_metadata.clone()
            },
            active_count: self.active_count,
            fresh_generation: self.fresh_generation,
            fresh: self.fresh,
            implicit_slot_metadata: self.implicit_slot_metadata,
            revision: self.revision,
            rewrite_revision: self.rewrite_revision,
        }
    }

    fn clone_from(&mut self, source: &Self) {
        self.bump_rewrite_revision();
        self.dummy.clone_from(&source.dummy);
        if source.implicit_slot_metadata {
            if let Some(metadata) = self.slot_metadata.as_mut() {
                metadata.generations.clear();
                metadata.occupied.clear();
                metadata.free_indices.clear();
            }
        } else if let Some(source_metadata) = source.slot_metadata.as_deref() {
            self.slot_metadata_mut().clone_from(source_metadata);
        }
        self.active_count = source.active_count;
        self.fresh_generation = source.fresh_generation;
        self.fresh = source.fresh;
        self.implicit_slot_metadata = source.implicit_slot_metadata;

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
                if let Some(metadata) = self.arena.slot_metadata.as_mut() {
                    metadata.generations.truncate(length);
                    metadata.occupied.truncate(length);
                    metadata.free_indices.retain(|arena_index| {
                        storage_index_from_arena_index(*arena_index) < length
                    });
                }
                self.arena.active_count = if self.arena.implicit_slot_metadata {
                    length
                } else {
                    self.arena
                        .slot_metadata()
                        .occupied
                        .iter()
                        .filter(|occupied| **occupied)
                        .count()
                };
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

impl<T> Arena<T> {
    /// The owned freshness counter, bumped by every mutation-capable call.
    /// A consumer comparing revisions across borrows sees any change to the
    /// arena's contents, including in-place rewrites that keep the length
    /// fixed — the case a length fingerprint cannot distinguish.
    #[inline]
    pub fn revision(&self) -> u64 {
        self.revision
    }

    #[inline]
    pub fn append_epoch(&self) -> Option<(u64, usize)> {
        self.fresh
            .then_some((self.rewrite_revision, self.items.len()))
    }

    #[inline]
    fn bump_revision(&mut self) {
        self.revision = self
            .revision
            .checked_add(1)
            .expect("arena revision overflow");
    }

    #[inline]
    fn bump_rewrite_revision(&mut self) {
        self.bump_revision();
        self.rewrite_revision = self
            .rewrite_revision
            .checked_add(1)
            .expect("arena rewrite revision overflow");
    }
}

impl<T: Default> Arena<T> {
    #[inline]
    fn append_slot_metadata(&mut self) {
        if !self.implicit_slot_metadata {
            let generation = self.fresh_generation;
            let metadata = self.slot_metadata_mut();
            metadata.generations.push(generation);
            metadata.occupied.push(true);
        }
    }

    fn materialize_slot_metadata(&mut self) {
        if self.implicit_slot_metadata && !self.items.is_empty() {
            let length = self.items.len();
            let generation = self.fresh_generation;
            let metadata = self.slot_metadata_mut();
            metadata.generations.resize(length, generation);
            metadata.occupied.resize(length, true);
            self.implicit_slot_metadata = false;
        }
    }

    pub fn new() -> Self {
        Self::with_capacity_and_generation(0, 1)
    }

    pub fn with_generation(generation: u32) -> Self {
        Self::with_capacity_and_generation(0, generation)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self::with_capacity_and_generation(capacity, 1)
    }

    pub fn with_capacity_and_generation(capacity: usize, generation: u32) -> Self {
        assert!(generation != 0, "arena fresh generation must be nonzero");
        Self {
            dummy: T::default(),
            items: Vec::with_capacity(capacity),
            slot_metadata: None,
            active_count: 0,
            fresh_generation: generation,
            fresh: true,
            implicit_slot_metadata: true,
            revision: 0,
            rewrite_revision: 0,
        }
    }

    pub fn reserve(&mut self, additional: usize) {
        self.items.reserve(additional);
        if !self.implicit_slot_metadata {
            self.slot_metadata_mut().generations.reserve(additional);
            self.slot_metadata_mut().occupied.reserve(additional);
        }
    }

    pub fn insert(&mut self, item: T) -> Handle<T> {
        self.bump_revision();
        if let Some(arena_index) = self
            .slot_metadata
            .as_mut()
            .and_then(|metadata| metadata.free_indices.pop())
        {
            self.rewrite_revision = self
                .rewrite_revision
                .checked_add(1)
                .expect("arena rewrite revision overflow");
            let index = storage_index_from_arena_index(arena_index);

            self.items[index] = item;
            self.slot_metadata_mut().occupied[index] = true;
            self.active_count = self
                .active_count
                .checked_add(1)
                .expect("arena active count overflow");

            return Handle::from_parts(arena_index, self.slot_metadata().generations[index]);
        }

        let arena_index = next_arena_index(self.items.len());

        self.items.push(item);
        self.append_slot_metadata();
        self.active_count = self
            .active_count
            .checked_add(1)
            .expect("arena active count overflow");

        Handle::from_parts(arena_index, self.fresh_generation)
    }

    pub fn append(&mut self, item: T) -> Handle<T> {
        self.bump_revision();
        let arena_index = next_arena_index(self.items.len());

        self.items.push(item);
        self.append_slot_metadata();
        self.active_count = self
            .active_count
            .checked_add(1)
            .expect("arena active count overflow");

        Handle::from_parts(arena_index, self.fresh_generation)
    }

    /// A span grows in place only while its rows still end at the arena tail
    /// carrying the arena's fresh generation. A
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
        self.bump_revision();
        if !span.is_empty()
            && (span.start().generation() != self.fresh_generation
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

    pub fn insert_many(&mut self, items: impl IntoIterator<Item = T>) -> HandleSpan<T> {
        // Spans promise contiguous storage, so bulk insert appends instead of
        // consuming arbitrary free-list slots.
        self.bump_revision();
        let start_index = next_arena_index(self.items.len());
        let mut count = 0u32;

        for item in items {
            self.items.push(item);
            self.append_slot_metadata();
            count = count.checked_add(1).expect("arena span count overflow");
        }

        if count == 0 {
            HandleSpan::empty()
        } else {
            self.active_count = self
                .active_count
                .checked_add(usize::try_from(count).expect("arena span count overflow"))
                .expect("arena active count overflow");

            HandleSpan::from_parts(
                Handle::from_parts(start_index, self.fresh_generation),
                count,
            )
        }
    }

    pub fn try_insert_many_with<E>(
        &mut self,
        insert_items: impl FnOnce(&mut ArenaSpanInserter<'_, T>) -> Result<(), E>,
    ) -> Result<HandleSpan<T>, E> {
        // Spans promise contiguous storage. This fallible variant lets
        // producers emit directly into the arena while preserving rollback.
        self.bump_revision();
        let start_len = self.items.len();
        let start_active_count = self.active_count;
        let start_index = next_arena_index(start_len);
        let mut inserter = ArenaSpanInserter {
            arena: self,
            count: 0,
        };

        if let Err(error) = insert_items(&mut inserter) {
            inserter.arena.items.truncate(start_len);
            if let Some(metadata) = inserter.arena.slot_metadata.as_mut() {
                metadata.generations.truncate(start_len);
                metadata.occupied.truncate(start_len);
            }
            inserter.arena.active_count = start_active_count;
            return Err(error);
        }

        if inserter.count == 0 {
            Ok(HandleSpan::empty())
        } else {
            Ok(HandleSpan::from_parts(
                Handle::from_parts(start_index, inserter.arena.fresh_generation),
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
        self.bump_revision();
        let start_len = self.items.len();
        let start_index = next_arena_index(start_len);
        let mut count = 0u32;

        for item in items {
            match item {
                Ok(item) => {
                    self.items.push(item);
                    self.append_slot_metadata();
                    count = count.checked_add(1).expect("arena span count overflow");
                }
                Err(error) => {
                    self.items.truncate(start_len);
                    if let Some(metadata) = self.slot_metadata.as_mut() {
                        metadata.generations.truncate(start_len);
                        metadata.occupied.truncate(start_len);
                    }
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
                Handle::from_parts(start_index, self.fresh_generation),
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
        self.bump_revision();

        let start_index = next_arena_index(self.items.len());

        if let Some(first_range) = first_range {
            for index in first_range {
                self.items.push(self.items[index].clone());
                self.append_slot_metadata();
            }
        }

        if let Some(second_range) = second_range {
            for index in second_range {
                self.items.push(self.items[index].clone());
                self.append_slot_metadata();
            }
        }

        self.active_count = self
            .active_count
            .checked_add(count)
            .expect("arena active count overflow");

        HandleSpan::from_parts(
            Handle::from_parts(start_index, self.fresh_generation),
            count.try_into().expect("arena span count overflow"),
        )
    }

    #[inline]
    pub fn get(&self, handle: Handle<T>) -> &T {
        let index = self.index_from_valid_handle(handle);

        self.items.get(index).unwrap_or_else(|| self.dummy())
    }

    pub fn get_mut(&mut self, handle: Handle<T>) -> &mut T {
        self.bump_rewrite_revision();
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
        self.bump_rewrite_revision();

        let replacement = T::default();
        self.materialize_slot_metadata();
        self.items[index] = replacement;
        self.slot_metadata_mut().occupied[index] = false;
        self.slot_metadata_mut().generations[index] =
            next_generation(self.slot_metadata().generations[index]);
        self.slot_metadata_mut()
            .free_indices
            .push(next_arena_index(index));
        self.active_count -= 1;
        self.fresh = false;

        true
    }

    #[inline]
    pub fn is_valid(&self, handle: Handle<T>) -> bool {
        self.index_from_valid_handle(handle) != invalid_index()
    }

    /// If `item` is a reference to an occupied element in this arena's slice,
    /// returns its `Handle<T>` in $O(1)$ by pointer-offset arithmetic.
    #[inline]
    pub fn handle_of(&self, item: &T) -> Option<Handle<T>> {
        let size = std::mem::size_of::<T>();
        if size == 0 {
            return None;
        }
        let base = self.items.as_ptr().addr();
        let addr = std::ptr::from_ref(item).addr();
        let offset_bytes = addr.checked_sub(base)?;
        let index = offset_bytes / size;
        let candidate = self.items.get(index)?;
        if !std::ptr::eq(candidate, item) {
            return None;
        }
        if self.fresh {
            return Some(Handle::from_parts(
                next_arena_index(index),
                self.fresh_generation,
            ));
        }
        self.slot_metadata()
            .occupied
            .get(index)
            .copied()
            .unwrap_or(false)
            .then(|| {
                Handle::from_parts(
                    next_arena_index(index),
                    self.slot_metadata().generations[index],
                )
            })
    }

    #[inline]
    pub fn span(&self, span: HandleSpan<T>) -> Option<&[T]> {
        if span.is_empty() {
            return Some(&[]);
        }

        let range = self.valid_span_range(span)?;

        self.items.get(range)
    }

    #[inline]
    pub fn span_or_empty(&self, span: HandleSpan<T>) -> &[T] {
        self.span(span).unwrap_or(&self.items[0..0])
    }

    #[inline]
    pub fn span_mut(&mut self, span: HandleSpan<T>) -> Option<&mut [T]> {
        self.bump_rewrite_revision();
        if span.is_empty() {
            return Some(&mut []);
        }

        let range = self.valid_span_range(span)?;

        self.items.get_mut(range)
    }

    #[inline]
    pub fn span_mut_or_empty(&mut self, span: HandleSpan<T>) -> &mut [T] {
        self.bump_rewrite_revision();
        if let Some(range) = self.valid_span_range(span) {
            &mut self.items[range]
        } else {
            &mut self.items[0..0]
        }
    }

    #[inline]
    fn valid_span_range(&self, span: HandleSpan<T>) -> Option<Range<usize>> {
        // A fresh arena has every slot occupied at its assigned generation, so the
        // start handle resolves to its index by arithmetic alone — no
        // per-slot metadata reads, and the whole-span check reduces to a
        // single bounds test.
        if self.fresh {
            let start_handle = span.start();
            if !start_handle.is_valid() || start_handle.generation() != self.fresh_generation {
                return None;
            }
            let start = storage_index_from_arena_index(start_handle.arena_index());
            let count = usize::try_from(span.count()).ok()?;
            let end = start.checked_add(count)?;
            return (end <= self.items.len()).then_some(start..end);
        }

        let start = self.index_from_valid_handle(span.start());
        if start == invalid_index() {
            return None;
        }

        let count = usize::try_from(span.count()).ok()?;
        let end = start.checked_add(count)?;
        let occupied = self.slot_metadata().occupied.get(start..end)?;
        let generations = self.slot_metadata().generations.get(start..end)?;

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

    #[inline]
    pub fn len(&self) -> usize {
        self.active_count
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn clear(&mut self) {
        self.bump_rewrite_revision();
        self.materialize_slot_metadata();
        if let Some(metadata) = self.slot_metadata.as_mut() {
            for (index, item) in self.items.iter_mut().enumerate() {
                *item = T::default();
                metadata.generations[index] = next_generation(metadata.generations[index]);
                metadata.occupied[index] = false;
            }
            metadata.free_indices.clear();
            metadata
                .free_indices
                .extend((0..self.items.len()).map(next_arena_index));
        }
        self.active_count = 0;
        self.fresh = self.items.is_empty();
    }

    /// Drop an unpublished append-only suffix, retaining allocation capacity.
    /// Callers must not publish suffix handles or mutate/free earlier rows.
    pub fn rewind_appended(&mut self, retained_length: usize) {
        assert!(self.fresh, "append rewind requires an unretired arena");
        assert!(
            retained_length <= self.items.len(),
            "append rewind exceeds arena"
        );
        self.bump_rewrite_revision();
        self.items.truncate(retained_length);
        if let Some(metadata) = self.slot_metadata.as_mut() {
            metadata.generations.truncate(retained_length);
            metadata.occupied.truncate(retained_length);
        }
        self.active_count = retained_length;
    }

    pub fn reset_retain_capacity(&mut self) {
        self.bump_rewrite_revision();
        self.items.clear();
        if let Some(metadata) = self.slot_metadata.as_mut() {
            metadata.generations.clear();
            metadata.occupied.clear();
            metadata.free_indices.clear();
        }
        self.active_count = 0;
        self.fresh = true;
        self.implicit_slot_metadata = true;
    }

    pub fn map<U: Default>(self, mut map_item: impl FnMut(T) -> U) -> Arena<U> {
        let mut items = Vec::with_capacity(self.items.len());
        items.extend(self.items.into_iter().map(&mut map_item));

        Arena {
            dummy: U::default(),
            items,
            slot_metadata: self.slot_metadata,
            active_count: self.active_count,
            fresh_generation: self.fresh_generation,
            fresh: self.fresh,
            implicit_slot_metadata: self.implicit_slot_metadata,
            revision: self.revision,
            rewrite_revision: self.rewrite_revision,
        }
    }

    pub fn storage_slice(&self) -> &[T] {
        &self.items
    }

    pub fn dummy(&self) -> &T {
        &self.dummy
    }

    #[inline]
    pub fn iter(&self) -> ArenaIter<'_, T> {
        ArenaIter {
            arena: self,
            index: 0,
            marker: PhantomData,
        }
    }

    #[inline]
    pub fn iter_from(&self, start: usize) -> ArenaIter<'_, T> {
        ArenaIter {
            arena: self,
            index: start.min(self.items.len()),
            marker: PhantomData,
        }
    }

    pub fn into_items(self) -> impl Iterator<Item = T> {
        let implicit = self.implicit_slot_metadata;
        // Generations and free slots no longer serve a consuming iterator.
        // Retain only occupancy, releasing the other cold buffers immediately.
        let occupied = self.slot_metadata.map(|metadata| metadata.occupied);
        self.items
            .into_iter()
            .enumerate()
            .filter_map(move |(index, item)| {
                (implicit || occupied.as_deref().unwrap_or(&[])[index]).then_some(item)
            })
    }

    pub fn for_each_mut(&mut self, mut visit: impl FnMut(Handle<T>, &mut T)) {
        self.bump_rewrite_revision();
        if self.implicit_slot_metadata {
            for (index, item) in self.items.iter_mut().enumerate() {
                visit(
                    Handle::from_parts(next_arena_index(index), self.fresh_generation),
                    item,
                );
            }
            return;
        }
        for index in 0..self.items.len() {
            if !self.slot_metadata().occupied[index] {
                continue;
            }

            let arena_index = next_arena_index(index);
            let handle = Handle::from_parts(arena_index, self.slot_metadata().generations[index]);

            visit(handle, &mut self.items[index]);
        }
    }

    #[inline]
    fn index_from_valid_handle(&self, handle: Handle<T>) -> usize {
        if !handle.is_valid() || handle.generation() == 0 {
            return invalid_index();
        }

        let index = storage_index_from_arena_index(handle.arena_index());

        // A fresh arena has every slot occupied at its assigned generation, so a handle
        // resolves by its generation and a bounds test alone, without reading
        // the per-slot metadata — the single-handle twin of the span check.
        if self.fresh {
            return if handle.generation() == self.fresh_generation && index < self.items.len() {
                index
            } else {
                invalid_index()
            };
        }

        if self
            .slot_metadata()
            .generations
            .get(index)
            .is_some_and(|generation| *generation == handle.generation())
            && self
                .slot_metadata()
                .occupied
                .get(index)
                .is_some_and(|occupied| *occupied)
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
        self.arena.append_slot_metadata();
        self.arena.active_count = self
            .arena
            .active_count
            .checked_add(1)
            .expect("arena active count overflow");
        self.count = self
            .count
            .checked_add(1)
            .expect("arena span count overflow");

        Handle::from_parts(arena_index, self.arena.fresh_generation)
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

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        // The borrowed arena cannot change under the iterator. A fresh one
        // has every slot occupied at its assigned generation, so each row is yielded
        // without reading the per-slot occupancy and generation metadata.
        if self.arena.fresh {
            let item = self.arena.items.get(self.index)?;
            let handle =
                Handle::from_parts(next_arena_index(self.index), self.arena.fresh_generation);
            self.index += 1;
            return Some((handle, item));
        }
        while self.index < self.arena.items.len() {
            let index = self.index;
            self.index += 1;

            if !self.arena.slot_metadata().occupied[index] {
                continue;
            }

            let arena_index = next_arena_index(index);
            let handle =
                Handle::from_parts(arena_index, self.arena.slot_metadata().generations[index]);

            return Some((handle, &self.arena.items[index]));
        }

        None
    }
}

#[inline]
fn invalid_index() -> usize {
    usize::MAX
}

#[inline]
fn next_arena_index(storage_len: usize) -> u32 {
    storage_len
        .checked_add(1)
        .and_then(|index| index.try_into().ok())
        .expect("arena index overflow")
}

#[inline]
fn storage_index_from_arena_index(arena_index: u32) -> usize {
    arena_index
        .checked_sub(1)
        .and_then(|index| usize::try_from(index).ok())
        .unwrap_or_else(invalid_index)
}

#[inline]
fn next_generation(generation: u32) -> u32 {
    let next = generation.wrapping_add(1);

    if next == 0 { 1 } else { next }
}

#[cfg(test)]
mod tests;
