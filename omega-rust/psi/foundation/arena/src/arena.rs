/*
We implement the contiguous `Arena`, its typed `Handle` and `HandleSpan` coordinates,
and the `HierarchyArena` and `OrderedRootArena` wrappers that organize parent-child
ranges and ordered declaration roots over dense storage.

An unretired `Arena` page has one generation and no holes, so we keep that uniform slot
identity on the arena header and resolve handles and spans by bounds arithmetic alone
until the first retirement materializes cold per-slot generation and occupancy vectors.
`HierarchyArena` assigns each parent one contiguous child `HandleSpan` in the shared
dense `Arena`, and `OrderedRootArena` maintains an ordered span of root handles while
preserving stable declaration identities in the backing `Arena`.
*/

use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::ops::{Index, IndexMut, Range};

pub struct Handle<T> {
    arena_index: u32,
    generation: u32,
    marker: PhantomData<fn() -> T>,
}

impl<T> Handle<T> {
    #[inline]
    pub const fn invalid() -> Self {
        Self {
            arena_index: 0,
            generation: 0,
            marker: PhantomData,
        }
    }

    #[inline]
    pub const fn from_arena_index(arena_index: u32) -> Self {
        Self {
            arena_index,
            generation: 1,
            marker: PhantomData,
        }
    }

    #[inline]
    pub const fn from_parts(arena_index: u32, generation: u32) -> Self {
        Self {
            arena_index,
            generation,
            marker: PhantomData,
        }
    }

    #[inline]
    pub const fn arena_index(self) -> u32 {
        self.arena_index
    }

    #[inline]
    pub const fn generation(self) -> u32 {
        self.generation
    }

    #[inline]
    pub const fn is_valid(self) -> bool {
        self.arena_index != 0
    }
}

impl<T> Clone for Handle<T> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Handle<T> {}

impl<T> Default for Handle<T> {
    #[inline]
    fn default() -> Self {
        Self::invalid()
    }
}

impl<T> PartialEq for Handle<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.arena_index == other.arena_index && self.generation == other.generation
    }
}

impl<T> Eq for Handle<T> {}

impl<T> Hash for Handle<T> {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.arena_index.hash(state);
        self.generation.hash(state);
    }
}

impl<T> fmt::Debug for Handle<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Handle")
            .field("arena_index", &self.arena_index)
            .field("generation", &self.generation)
            .finish()
    }
}

pub fn unique_handles_in_order<T>(handles: impl IntoIterator<Item = Handle<T>>) -> Vec<Handle<T>> {
    let mut ordered: Vec<_> = handles
        .into_iter()
        .enumerate()
        .map(|(order, handle)| (handle, order))
        .collect();
    ordered.sort_by_key(|(handle, _)| (handle.arena_index(), handle.generation()));
    ordered.dedup_by_key(|(handle, _)| (handle.arena_index(), handle.generation()));
    ordered.sort_by_key(|(_, order)| *order);
    ordered.into_iter().map(|(handle, _)| handle).collect()
}

impl<T: 'static> compiler_storage::Record for Handle<T> {
    fn write(
        &self,
        output: &mut compiler_storage::RecordWriter,
    ) -> Result<(), compiler_storage::RecordError> {
        self.arena_index.write(output)?;
        self.generation.write(output)
    }
    fn read<Context>(
        input: &mut compiler_storage::RecordReader<'_, Context>,
    ) -> Result<Self, compiler_storage::RecordError> {
        let arena_index = u32::read(input)?;
        let generation = u32::read(input)?;
        let generation = if arena_index == 0 {
            generation
        } else {
            input.generation::<T>(generation)?
        };
        Ok(Self::from_parts(arena_index, generation))
    }
}

pub struct HandleSpan<T> {
    start: Handle<T>,
    count: u32,
    marker: PhantomData<fn() -> T>,
}

impl<T> HandleSpan<T> {
    pub const fn empty() -> Self {
        Self {
            start: Handle::invalid(),
            count: 0,
            marker: PhantomData,
        }
    }

    pub const fn from_parts(start: Handle<T>, count: u32) -> Self {
        Self {
            start,
            count,
            marker: PhantomData,
        }
    }

    pub const fn from_parts_or_empty(start: Handle<T>, count: u32) -> Self {
        if count == 0 {
            Self::empty()
        } else {
            Self::from_parts(start, count)
        }
    }

    pub const fn start(self) -> Handle<T> {
        self.start
    }

    pub const fn count(self) -> u32 {
        self.count
    }

    pub const fn len(self) -> usize {
        self.count as usize
    }

    pub const fn is_empty(self) -> bool {
        self.count == 0
    }

    pub fn handle_at(self, offset: u32) -> Handle<T> {
        Handle::from_parts(
            self.start
                .arena_index()
                .checked_add(offset)
                .expect("handle span index overflow"),
            self.start.generation(),
        )
    }

    pub fn handles(self) -> SpanHandles<T> {
        SpanHandles {
            span: self,
            offset: 0,
        }
    }

    pub fn is_followed_by(self, handle: Handle<T>) -> bool {
        !self.is_empty()
            && handle.generation() == self.start.generation()
            && handle.arena_index() == self.start.arena_index() + self.count
    }

    pub fn push_contiguous(&mut self, handle: Handle<T>) {
        if self.is_empty() {
            *self = Self::from_parts(handle, 1);
            return;
        }

        let expected_index = self
            .start
            .arena_index()
            .checked_add(self.count)
            .expect("handle span index overflow");
        assert_eq!(
            handle.arena_index(),
            expected_index,
            "handle span push must be contiguous"
        );
        assert_eq!(
            handle.generation(),
            self.start.generation(),
            "handle span push must keep one generation"
        );
        self.count = self
            .count
            .checked_add(1)
            .expect("handle span count overflow");
    }
}

#[derive(Debug)]
pub struct SpanHandles<T> {
    span: HandleSpan<T>,
    offset: u32,
}

impl<T> SpanHandles<T> {
    pub const fn empty() -> Self {
        Self {
            span: HandleSpan::empty(),
            offset: 0,
        }
    }

    #[inline]
    fn remaining(&self) -> u32 {
        self.span.count() - self.offset
    }
}

impl<T> Clone for SpanHandles<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for SpanHandles<T> {}

impl<T> Iterator for SpanHandles<T> {
    type Item = Handle<T>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.offset >= self.span.count() {
            return None;
        }

        let handle = self.span.handle_at(self.offset);
        self.offset += 1;

        Some(handle)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.remaining() as usize;
        (remaining, Some(remaining))
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        match u32::try_from(n) {
            Ok(skip) if skip < self.remaining() => {
                self.offset += skip;
                self.next()
            }
            _ => {
                self.offset = self.span.count();
                None
            }
        }
    }
}

impl<T> ExactSizeIterator for SpanHandles<T> {
    #[inline]
    fn len(&self) -> usize {
        self.remaining() as usize
    }
}

impl<T> std::iter::FusedIterator for SpanHandles<T> {}

impl<T> Clone for HandleSpan<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for HandleSpan<T> {}

impl<T> Default for HandleSpan<T> {
    fn default() -> Self {
        Self::empty()
    }
}

impl<T> PartialEq for HandleSpan<T> {
    fn eq(&self, other: &Self) -> bool {
        self.start == other.start && self.count == other.count
    }
}

impl<T> Eq for HandleSpan<T> {}

impl<T> std::hash::Hash for HandleSpan<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.start.hash(state);
        self.count.hash(state);
    }
}

impl<T> fmt::Debug for HandleSpan<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HandleSpan")
            .field("start", &self.start)
            .field("count", &self.count)
            .finish()
    }
}

impl<T: 'static> compiler_storage::Record for HandleSpan<T> {
    fn write(
        &self,
        output: &mut compiler_storage::RecordWriter,
    ) -> Result<(), compiler_storage::RecordError> {
        self.start.write(output)?;
        self.count.write(output)
    }
    fn read<Context>(
        input: &mut compiler_storage::RecordReader<'_, Context>,
    ) -> Result<Self, compiler_storage::RecordError> {
        let start = Handle::read(input)?;
        let count = u32::read(input)?;
        if count != 0 && (!start.is_valid() || start.arena_index().checked_add(count - 1).is_none())
        {
            return Err(input.error("arena span has an invalid start or overflowing range"));
        }
        Ok(Self::from_parts(start, count))
    }
}

#[derive(Default, compiler_storage::Record)]
pub struct ArenaSlotStorage {
    pub generations: Vec<u32>,
    pub occupied: Vec<bool>,
    pub free_indices: Vec<u32>,
}

type SlotMetadata = ArenaSlotStorage;

pub struct ArenaStorageView<'storage, T> {
    pub dummy: &'storage T,
    pub items: &'storage [T],
    pub slots: Option<&'storage ArenaSlotStorage>,
    pub fresh_generation: u32,
    pub fresh: bool,
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
    fresh_generation: u32, // Generation assigned to every newly appended row.
    fresh: bool, // True while no slot has been freed or cleared since the arena was last empty.
    implicit_slot_metadata: bool, // Physical storage mode separate from logical `fresh`.
    revision: u64, // Monotonic mutation counter across inserts, frees, and mutable borrows.
    rewrite_revision: u64, // Monotonic counter bumped on non-append rewrites and frees.
}

impl<T> Arena<T> {
    pub fn storage_view(&self) -> ArenaStorageView<'_, T> {
        ArenaStorageView {
            dummy: &self.dummy,
            items: &self.items,
            slots: (!self.implicit_slot_metadata).then(|| self.slot_metadata()),
            fresh_generation: self.fresh_generation,
            fresh: self.fresh,
        }
    }

    pub fn from_storage(
        dummy: T,
        items: Vec<T>,
        slots: Option<ArenaSlotStorage>,
        fresh_generation: u32,
        fresh: bool,
    ) -> Result<Self, &'static str> {
        if fresh_generation == 0 || items.len() > u32::MAX as usize {
            return Err("arena storage has an invalid generation or row count");
        }
        let active_count = if let Some(slots) = &slots {
            if slots.generations.len() != items.len() || slots.occupied.len() != items.len() {
                return Err("arena slot roster does not match payload storage");
            }
            if slots.generations.contains(&0) {
                return Err("arena slot roster has a zero generation");
            }
            let mut vacant = slots
                .occupied
                .iter()
                .map(|occupied| !occupied)
                .collect::<Vec<_>>();
            for &arena_index in &slots.free_indices {
                let Some(unlisted) = arena_index
                    .checked_sub(1)
                    .and_then(|index| vacant.get_mut(index as usize))
                else {
                    return Err("arena free list has an out-of-range slot");
                };
                if !*unlisted {
                    return Err("arena free list repeats a slot or includes an occupied slot");
                }
                *unlisted = false;
            }
            if vacant.contains(&true) {
                return Err("arena free list omits a vacant slot");
            }
            if fresh
                && (slots.occupied.contains(&false)
                    || slots
                        .generations
                        .iter()
                        .any(|generation| *generation != fresh_generation))
            {
                return Err("arena fresh storage disagrees with its slot roster");
            }
            items.len() - slots.free_indices.len()
        } else {
            if !fresh {
                return Err("arena nonuniform storage is missing its slot roster");
            }
            items.len()
        };
        Ok(Self {
            dummy,
            items,
            implicit_slot_metadata: slots.is_none(),
            slot_metadata: slots.map(Box::new),
            active_count,
            fresh_generation,
            fresh,
            revision: 0,
            rewrite_revision: 0,
        })
    }

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
        self.try_get(handle).unwrap_or_else(|| self.dummy())
    }

    /// Resolves a live row without substituting the shared default. Zero,
    /// retired generations and out-of-range handles return `None`; ownership
    /// by a particular arena still requires its owning context.
    #[inline]
    pub fn try_get(&self, handle: Handle<T>) -> Option<&T> {
        self.items.get(self.index_from_valid_handle(handle))
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

    /// Reuse dense storage without allowing a retired handle to name a new row.
    pub fn retire_all_retain_capacity(&mut self) {
        let latest_generation = self
            .slot_metadata()
            .generations
            .iter()
            .copied()
            .fold(self.fresh_generation, u32::max);
        let generation = latest_generation
            .checked_add(1)
            .expect("arena generation exhausted during dense retirement");
        self.reset_retain_capacity();
        self.fresh_generation = generation;
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

impl<T: compiler_storage::Record> compiler_storage::Record for Arena<T> {
    fn write(
        &self,
        output: &mut compiler_storage::RecordWriter,
    ) -> Result<(), compiler_storage::RecordError> {
        output.record(|output| {
            let storage = self.storage_view();
            storage.dummy.write(output)?;
            output.record(|output| {
                output.sequence(storage.items.len())?;
                for item in storage.items {
                    item.write(output)?;
                }
                Ok(())
            })?;
            match storage.slots {
                Some(slots) => {
                    true.write(output)?;
                    slots.write(output)?;
                }
                None => false.write(output)?,
            }
            storage.fresh_generation.write(output)?;
            storage.fresh.write(output)
        })
    }
    fn read<Context>(
        input: &mut compiler_storage::RecordReader<'_, Context>,
    ) -> Result<Self, compiler_storage::RecordError> {
        input.record(|input| {
            let dummy = T::read(input)?;
            let items = Vec::<T>::read(input)?;
            let slots = if bool::read(input)? {
                Some(ArenaSlotStorage::read(input)?)
            } else {
                None
            };
            Self::from_storage(dummy, items, slots, u32::read(input)?, bool::read(input)?)
                .map_err(|reason| input.error(reason))
        })
    }
}

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

    pub fn get_mut(&mut self, node: Handle<T>) -> &mut T {
        self.nodes.get_mut(node)
    }

    pub fn insert_root(&mut self, mut node: T) -> Handle<T> {
        node.set_parent(Handle::invalid());
        node.set_children(HandleSpan::empty());

        self.nodes.append(node)
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

    pub fn insert_supplemental_child(&mut self, parent: Handle<T>, mut node: T) -> Handle<T> {
        node.set_parent(parent);
        node.set_children(HandleSpan::empty());

        self.nodes.append(node)
    }

    pub fn children(&self, parent: Handle<T>) -> Option<&[T]> {
        self.nodes.span(self.get(parent).children())
    }

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

impl<T: HierarchyNode + compiler_storage::Record> compiler_storage::Record for HierarchyArena<T> {
    fn write(
        &self,
        output: &mut compiler_storage::RecordWriter,
    ) -> Result<(), compiler_storage::RecordError> {
        self.nodes.write(output)
    }
    fn read<Context>(
        input: &mut compiler_storage::RecordReader<'_, Context>,
    ) -> Result<Self, compiler_storage::RecordError> {
        let nodes = Arena::<T>::read(input)?;
        for (handle, node) in nodes.iter() {
            if node.parent().is_valid() && !nodes.is_valid(node.parent()) {
                return Err(input.error("hierarchy parent does not name live storage"));
            }
            let children = nodes
                .span(node.children())
                .ok_or_else(|| input.error("hierarchy child range does not name live storage"))?;
            if children.iter().any(|child| child.parent() != handle) {
                return Err(input.error("hierarchy child range disagrees with its parent"));
            }
        }
        let mut completed = vec![false; nodes.storage_slice().len()];
        let mut visiting = vec![false; completed.len()];
        for (handle, _) in nodes.iter() {
            let mut chain = Vec::new();
            let mut current = handle;
            while current.is_valid() {
                let position = current.arena_index() as usize - 1;
                if completed[position] {
                    break;
                }
                if visiting[position] {
                    return Err(input.error("hierarchy parent chain is cyclic"));
                }
                visiting[position] = true;
                chain.push(position);
                current = nodes.get(current).parent();
            }
            for position in chain {
                visiting[position] = false;
                completed[position] = true;
            }
        }
        Ok(Self { nodes })
    }
}

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

    pub fn get(&self, handle: Handle<T>) -> &T {
        self.storage.get(handle)
    }

    pub fn get_mut(&mut self, handle: Handle<T>) -> &mut T {
        self.storage.get_mut(handle)
    }

    pub fn contains(&self, handle: Handle<T>) -> bool {
        self.handles.span_or_empty(self.roots).contains(&handle)
    }

    pub fn iter(&self) -> OrderedRootArenaIter<'_, T> {
        OrderedRootArenaIter {
            storage: &self.storage,
            handles: self.handles.span_or_empty(self.roots).iter(),
            marker: PhantomData,
        }
    }

    /// Read exact retained declaration metadata, including records no longer
    /// present in the live root roster. This does not restore root membership.
    pub fn retained_records(&self) -> impl Iterator<Item = &T> {
        self.storage.iter().map(|(_, record)| record)
    }

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

impl<T: Default + compiler_storage::Record + 'static> compiler_storage::Record
    for OrderedRootArena<T>
{
    fn write(
        &self,
        output: &mut compiler_storage::RecordWriter,
    ) -> Result<(), compiler_storage::RecordError> {
        output.record(|output| {
            self.handles.write(output)?;
            self.roots.write(output)?;
            self.storage.write(output)
        })
    }
    fn read<Context>(
        input: &mut compiler_storage::RecordReader<'_, Context>,
    ) -> Result<Self, compiler_storage::RecordError> {
        input.record(|input| {
            let result = Self {
                handles: Arena::read(input)?,
                roots: HandleSpan::read(input)?,
                storage: Arena::read(input)?,
            };
            let roots = result
                .handles
                .span(result.roots)
                .ok_or_else(|| input.error("ordered roots have no live roster"))?;
            if roots.iter().any(|handle| !result.storage.is_valid(*handle)) {
                return Err(input.error("ordered root does not name live declaration storage"));
            }
            let mut identities = roots.to_vec();
            identities.sort_unstable_by_key(|handle| (handle.arena_index(), handle.generation()));
            if identities.windows(2).any(|pair| pair[0] == pair[1]) {
                return Err(input.error("ordered roots repeat a declaration"));
            }
            Ok(result)
        })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn removed_ordered_roots_retain_metadata_without_membership() {
        let mut roots = super::OrderedRootArena::new();
        let original = roots.push(String::from("original"));
        roots.push(String::from("selected"));
        roots.retain(|value| value == "selected");
        let copy = roots.clone();
        assert!(!copy.contains(original));
        assert_eq!(
            copy.iter().map(String::as_str).collect::<Vec<_>>(),
            ["selected"]
        );
        assert_eq!(copy.get(original), "original");
        assert_eq!(
            copy.retained_records()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["original", "selected"]
        );
    }

    use std::collections::HashSet;

    use super::{
        Arena, ArenaSlotStorage, Handle, HandleSpan, HierarchyArena, HierarchyNode,
        OrderedRootArena, storage_index_from_arena_index, unique_handles_in_order,
    };

    struct NonHashItem;

    #[test]
    fn hash_uses_the_complete_untyped_handle_identity() {
        let first = Handle::<NonHashItem>::from_parts(7, 2);
        let same = Handle::<NonHashItem>::from_parts(7, 2);
        let next_generation = Handle::<NonHashItem>::from_parts(7, 3);

        assert_eq!(HashSet::from([first, same, next_generation]).len(), 2);
    }

    #[test]
    fn unique_handles_preserve_generations_and_first_appearance() {
        let handles = [
            Handle::<NonHashItem>::from_parts(8, 1),
            Handle::from_parts(7, 2),
            Handle::from_parts(7, 1),
            Handle::invalid(),
        ];
        for length in 0..=5 {
            for sequence_number in 0..handles.len().pow(length) {
                let mut digits = sequence_number;
                let mut sequence = Vec::new();
                for _ in 0..length {
                    sequence.push(handles[digits % handles.len()]);
                    digits /= handles.len();
                }
                let mut expected = Vec::new();
                for handle in &sequence {
                    if !expected.contains(handle) {
                        expected.push(*handle);
                    }
                }
                assert_eq!(unique_handles_in_order(sequence), expected);
            }
        }
    }

    #[test]
    fn pushes_contiguous_handles() {
        let mut span = HandleSpan::<String>::empty();

        span.push_contiguous(Handle::from_arena_index(4));
        span.push_contiguous(Handle::from_arena_index(5));

        assert_eq!(span.start(), Handle::from_arena_index(4));
        assert_eq!(span.count(), 2);
    }

    #[test]
    #[should_panic(expected = "handle span push must be contiguous")]
    fn panics_on_non_contiguous_handles() {
        let mut span = HandleSpan::<String>::empty();

        span.push_contiguous(Handle::from_arena_index(4));
        span.push_contiguous(Handle::from_arena_index(6));
    }

    #[test]
    #[should_panic(expected = "handle span push must keep one generation")]
    fn panics_on_mixed_generation_handles() {
        let mut span = HandleSpan::<String>::empty();

        span.push_contiguous(Handle::from_arena_index(4));
        span.push_contiguous(Handle::from_parts(5, 2));
    }

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
    fn checked_lookup_does_not_substitute_dummy_for_missing_storage() {
        let mut arena = Arena::new();
        let handle = arena.insert(7u32);
        *arena.get_mut(Handle::invalid()) = 99;
        assert_eq!(arena.try_get(handle), Some(&7));
        assert_eq!(arena.try_get(Handle::invalid()), None);
        assert_eq!(
            arena.try_get(Handle::from_parts(handle.arena_index(), 9)),
            None
        );
        assert_eq!(
            arena.try_get(Handle::from_parts(99, handle.generation())),
            None
        );
        assert!(arena.free(handle));
        let replacement = arena.insert(8);
        assert_eq!(arena.try_get(handle), None);
        assert_eq!(arena.try_get(replacement), Some(&8));
        assert_eq!(arena.get(Handle::invalid()), &99);
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
    fn relocates_stale_spans_to_the_tail_before_appending() {
        let mut arena = Arena::new();
        let mut span = HandleSpan::empty();
        arena.append_to_span(&mut span, "alpha".to_owned());
        let gap = arena.append("gap".to_owned());

        let beta = arena.append_to_span(&mut span, "beta".to_owned());

        assert_eq!(beta.arena_index(), 4);
        assert_eq!(span.start().arena_index(), 3);
        assert_eq!(span.count(), 2);
        assert_eq!(
            arena.span(span).expect("span should resolve"),
            &["alpha".to_owned(), "beta".to_owned()]
        );
        assert_eq!(arena.get(gap).as_str(), "gap");
    }

    #[test]
    #[should_panic(expected = "arena span append on an unresolvable span")]
    fn panics_when_extending_a_span_with_freed_rows() {
        let mut arena = Arena::new();
        let mut span = HandleSpan::empty();
        let alpha = arena.append_to_span(&mut span, "alpha".to_owned());
        assert!(arena.free(alpha));
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

    #[test]
    fn clone_from_preserves_free_slots_generations_and_later_operations() {
        let mut source = Arena::new();
        let first = source.insert("alpha".to_owned());
        let retired = source.insert("beta".to_owned());
        let free = source.insert("gamma".to_owned());
        assert!(source.free(retired));
        let replacement = source.insert("replacement".to_owned());
        assert_ne!(replacement.generation(), retired.generation());
        assert!(source.free(free));
        *source.get_mut(Handle::invalid()) = "source dummy".to_owned();

        let mut copied = Arena::new();
        copied.insert("old".to_owned());
        copied.clone_from(&source);
        let mut expected = source.clone();
        assert_eq!(copied, expected);
        assert!(!copied.is_valid(retired));
        assert!(!copied.is_valid(free));
        assert_eq!(copied.get(replacement), source.get(replacement));
        assert_eq!(copied.get(retired), source.dummy());

        assert_eq!(
            copied.insert("reuse".to_owned()),
            expected.insert("reuse".to_owned())
        );
        assert_eq!(
            copied.append("append".to_owned()),
            expected.append("append".to_owned())
        );
        assert_eq!(copied.free(first), expected.free(first));
        assert_eq!(copied, expected);

        copied.get_mut(replacement).push_str(" changed");
        assert_eq!(source.get(replacement), "replacement");
        copied.clone_from(&Arena::default());
        assert_eq!(copied, Arena::default());
        copied.clone_from(&source);
        assert_eq!(copied, source.clone());
    }

    #[test]
    fn clone_from_reuses_storage_and_nested_payload_capacity_when_source_fits() {
        let mut source = Arena::new();
        source.append(vec!["short".to_owned()]);
        let free = source.append(vec!["unused".to_owned()]);
        assert!(source.free(free));
        *source.get_mut(Handle::invalid()) = vec!["dummy".to_owned()];

        let mut copied = Arena::with_capacity(8);
        copied.slot_metadata_mut().free_indices.reserve(8);
        for _ in 0..4 {
            copied.append(vec!["longer destination payload".to_owned()]);
        }
        let retired = copied.append(vec!["retired destination payload".to_owned()]);
        assert!(copied.free(retired));
        *copied.get_mut(Handle::invalid()) = vec!["longer destination dummy".to_owned()];
        let storage_pointers = (
            copied.items.as_ptr(),
            copied.slot_metadata().generations.as_ptr(),
            copied.slot_metadata().occupied.as_ptr(),
            copied.slot_metadata().free_indices.as_ptr(),
        );
        let capacities = (
            copied.items.capacity(),
            copied.slot_metadata().generations.capacity(),
            copied.slot_metadata().occupied.capacity(),
            copied.slot_metadata().free_indices.capacity(),
        );
        let nested_pointer = copied.items[0].as_ptr();
        let payload_pointer = copied.items[0][0].as_ptr();
        let dummy_pointer = copied.dummy[0].as_ptr();

        copied.clone_from(&source);

        assert_eq!(copied, source.clone());
        assert_eq!(
            storage_pointers,
            (
                copied.items.as_ptr(),
                copied.slot_metadata().generations.as_ptr(),
                copied.slot_metadata().occupied.as_ptr(),
                copied.slot_metadata().free_indices.as_ptr(),
            )
        );
        assert_eq!(
            capacities,
            (
                copied.items.capacity(),
                copied.slot_metadata().generations.capacity(),
                copied.slot_metadata().occupied.capacity(),
                copied.slot_metadata().free_indices.capacity(),
            )
        );
        assert_eq!(nested_pointer, copied.items[0].as_ptr());
        assert_eq!(payload_pointer, copied.items[0][0].as_ptr());
        assert_eq!(dummy_pointer, copied.dummy[0].as_ptr());
        copied.items[0][0].push_str(" changed");
        assert_eq!(source.items[0][0], "short");
    }

    #[derive(Debug, Default, PartialEq, Eq)]
    struct PanicOnClone {
        value: u32,
        panic_on_clone: bool,
    }

    impl Clone for PanicOnClone {
        fn clone(&self) -> Self {
            assert!(!self.panic_on_clone, "requested payload clone panic");
            Self {
                value: self.value,
                panic_on_clone: false,
            }
        }
    }

    fn assert_usable_after_clone_panic(arena: &mut Arena<PanicOnClone>) {
        assert_eq!(arena.items.len(), arena.slot_metadata().generations.len());
        assert_eq!(arena.items.len(), arena.slot_metadata().occupied.len());
        assert_eq!(arena.len(), arena.iter().count());
        for arena_index in &arena.slot_metadata().free_indices {
            let index = storage_index_from_arena_index(*arena_index);
            assert!(!arena.slot_metadata().occupied[index]);
        }
        for (handle, value) in arena.iter() {
            assert!(arena.is_valid(handle));
            assert_eq!(arena.get(handle), value);
        }
        let count = arena.len();
        let inserted = arena.insert(PanicOnClone {
            value: 100,
            ..Default::default()
        });
        let appended = arena.append(PanicOnClone {
            value: 101,
            ..Default::default()
        });
        assert_eq!(arena.len(), count + 2);
        assert_eq!(arena.get(inserted).value, 100);
        assert_eq!(arena.get(appended).value, 101);
        assert!(arena.free(inserted));
        assert!(!arena.is_valid(inserted));
        assert_eq!(arena.len(), count + 1);
        assert_eq!(arena.len(), arena.iter().count());
    }

    #[test]
    fn clone_from_payload_panic_after_shrinking_preserves_arena_structure() {
        let mut source = Arena::new();
        source.append(PanicOnClone::default());
        source.append(PanicOnClone {
            value: 1,
            panic_on_clone: true,
        });
        let free = source.append(PanicOnClone::default());
        assert!(source.free(free));
        let mut copied = Arena::new();
        for _ in 0..6 {
            copied.append(PanicOnClone::default());
        }

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            copied.clone_from(&source);
        }));

        assert!(result.is_err());
        assert_eq!(copied.items.len(), 3);
        assert_eq!(
            copied.slot_metadata().generations,
            source.slot_metadata().generations
        );
        assert_eq!(
            copied.slot_metadata().free_indices,
            source.slot_metadata().free_indices
        );
        assert_usable_after_clone_panic(&mut copied);
    }

    #[test]
    fn clone_from_payload_panic_after_partial_growth_preserves_arena_structure() {
        let mut source = Arena::new();
        for value in 0..6 {
            source.append(PanicOnClone {
                value,
                panic_on_clone: value == 2,
            });
        }
        assert!(source.free(Handle::from_arena_index(5)));
        let mut copied = Arena::new();
        copied.append(PanicOnClone::default());

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            copied.clone_from(&source);
        }));

        assert!(result.is_err());
        assert_eq!(copied.items.len(), 2);
        assert!(copied.slot_metadata().free_indices.is_empty());
        assert_usable_after_clone_panic(&mut copied);
    }

    #[test]
    fn span_resolution_rechecks_rows_once_a_slot_was_freed() {
        let mut arena = Arena::new();
        let span = arena.insert_many(["alpha".to_owned(), "beta".to_owned(), "gamma".to_owned()]);
        let past_end = HandleSpan::from_parts(span.start(), 4);
        assert_eq!(arena.span(span).map(<[String]>::len), Some(3));
        assert!(
            arena.span(past_end).is_none(),
            "a fresh arena still bounds spans"
        );

        let middle = span.handle_at(1);
        assert!(arena.free(middle));
        assert!(arena.span(span).is_none(), "a freed row breaks the span");

        arena.insert("bravo".to_owned());
        assert!(
            arena.span(span).is_none(),
            "a row recycled at a later generation does not rejoin the span"
        );

        arena.reset_retain_capacity();
        let fresh = arena.insert_many(["delta".to_owned(), "echo".to_owned()]);
        assert_eq!(arena.span(fresh).map(<[String]>::len), Some(2));
    }

    #[test]
    fn dense_retirement_reuses_capacity_without_reusing_any_slot_generation() {
        let mut arena = Arena::with_capacity_and_generation(8, 17);
        let original = arena.insert_many([10_u32, 20, 30]);
        let freed = original.handle_at(1);
        assert!(arena.free(freed));
        let replacement = arena.insert(40);
        let capacity = arena.items.capacity();

        arena.retire_all_retain_capacity();
        let current = arena.insert_many([50, 60, 70]);
        assert_eq!(arena.items.capacity(), capacity);
        assert_eq!(current.start().arena_index(), 1);
        assert!(arena.span(original).is_none());
        for old in original.handles().chain([replacement]) {
            assert!(!arena.is_valid(old));
        }
        assert_eq!(arena.span(current), Some([50, 60, 70].as_slice()));

        arena.retire_all_retain_capacity();
        let next = arena.insert(80);
        assert_eq!(next.arena_index(), 1);
        assert!(!arena.is_valid(current.start()));
        assert_eq!(*arena.get(next), 80);
    }

    #[test]
    fn uniform_page_retirement_preserves_unretired_handles_and_rejects_stale_slot() {
        let mut arena = Arena::with_capacity_and_generation(8, 17);
        let span = arena.insert_many([10_u32, 20, 30]);
        let first = span.handle_at(0);
        let retired = span.handle_at(1);
        let last = span.handle_at(2);
        assert_eq!(arena.clone().into_items().collect::<Vec<_>>(), [10, 20, 30]);
        arena.for_each_mut(|_, value| *value += 1);
        assert!(arena.free(retired));
        let replacement = arena.insert(42);
        assert_eq!(replacement.arena_index(), retired.arena_index());
        assert_ne!(replacement.generation(), retired.generation());
        assert!(!arena.is_valid(retired));
        assert_eq!(*arena.get(first), 11);
        assert_eq!(*arena.get(last), 31);
        assert!(arena.span(span).is_none());
        let copy = arena.clone().map(u64::from);
        assert_eq!(copy.into_items().collect::<Vec<_>>(), [11, 42, 31]);
        arena.clear();
        for handle in [first, retired, last, replacement] {
            assert!(!arena.is_valid(handle));
        }
        assert_eq!(arena.into_items().count(), 0);
    }

    #[test]
    fn uniform_source_clone_unwind_keeps_the_initialized_prefix_usable() {
        let mut source = Arena::new();
        for value in 0..6 {
            source.append(PanicOnClone {
                value,
                panic_on_clone: value == 2,
            });
        }
        let mut copied = Arena::new();
        let retired = copied.insert(PanicOnClone::default());
        copied.free(retired);
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| copied.clone_from(&source)));
        assert!(result.is_err());
        assert_eq!(
            copied
                .iter()
                .map(|(_, value)| value.value)
                .collect::<Vec<_>>(),
            [0, 1]
        );
        assert_eq!(copied.len(), 2);
        let handle = copied.insert(PanicOnClone {
            value: 10,
            ..Default::default()
        });
        assert_eq!(copied.get(handle).value, 10);
        assert!(copied.free(handle));
        assert!(!copied.is_valid(handle));
    }

    #[test]
    fn failed_first_retirement_preserves_logical_equality() {
        #[derive(Clone, Debug, PartialEq)]
        struct Payload(u32);
        thread_local! { static FAIL_DEFAULT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
        impl Default for Payload {
            fn default() -> Self {
                assert!(
                    !FAIL_DEFAULT.with(|flag| flag.get()),
                    "requested default panic"
                );
                Self(0)
            }
        }
        for clear_all in [false, true] {
            let mut rows = Arena::new();
            let handle = rows.append(Payload(42));
            let expected = rows.clone();
            FAIL_DEFAULT.with(|flag| flag.set(true));
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                if clear_all {
                    rows.clear();
                } else {
                    rows.free(handle);
                }
            }));
            FAIL_DEFAULT.with(|flag| flag.set(false));
            assert!(result.is_err());
            assert_eq!(rows, expected);
            assert!(rows.equal_by(&expected, |left, right| left == right));
            assert!(expected.equal_by(&rows, |left, right| left == right));
            rows.rewind_appended(1);
            let next = rows.append(Payload(7));
            assert_eq!(rows.get(handle), &Payload(42));
            assert_eq!(rows.get(next), &Payload(7));
            assert!(rows.free(handle));
            assert!(!rows.is_valid(handle));
        }
    }

    #[test]
    fn recovered_storage_preserves_retirement_and_next_allocation() {
        let mut original = Arena::<u32>::with_generation(7);
        let first = original.insert(11);
        let second = original.insert(22);
        let third = original.insert(33);
        assert!(original.free(first));
        assert!(original.free(third));
        let view = original.storage_view();
        let mut recovered = Arena::from_storage(
            *view.dummy,
            view.items.to_vec(),
            view.slots.cloned(),
            view.fresh_generation,
            view.fresh,
        )
        .expect("valid storage");
        assert_eq!(original, recovered);
        assert!(!recovered.is_valid(first));
        assert!(!recovered.is_valid(third));
        assert_eq!(recovered.get(second), original.get(second));
        for value in [44, 55, 66] {
            let expected = original.insert(value);
            assert_eq!(recovered.insert(value), expected);
            assert_eq!(original, recovered);
        }
        assert!(!recovered.is_valid(first));
        assert!(!recovered.is_valid(third));
    }

    #[test]
    fn recovered_storage_rejects_incomplete_or_repeated_free_roster() {
        for free_indices in [vec![], vec![1, 1], vec![0], vec![2]] {
            let slots = ArenaSlotStorage {
                generations: vec![2],
                occupied: vec![false],
                free_indices,
            };
            assert!(Arena::from_storage(0u32, vec![0], Some(slots), 1, false).is_err());
        }
    }

    #[test]
    fn recovered_storage_refuses_a_false_uniform_generation_claim() {
        let slots = ArenaSlotStorage {
            generations: vec![3],
            occupied: vec![true],
            free_indices: vec![],
        };
        assert!(Arena::from_storage(0u32, vec![10], Some(slots), 1, true).is_err());
    }

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
        assert_eq!(children.next(), None);
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
