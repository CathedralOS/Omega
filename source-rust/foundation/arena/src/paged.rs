/*
The generation in a PagedArenaSet handle selects its dense Arena page.
Checkpoint forks share completed pages through Arc and copy a page on its
first edit. The interners below assign durable keys to values and text.
*/

use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use crate::arena::{RevisionStamp, refuse_unresolved, refuse_unresolved_span};
use crate::{Arena, ArenaSpanInserter, Handle, HandleSpan};

enum ArenaPage<T> {
    Owned(Arena<T>),
    Shared {
        arena: Arc<Arena<T>>,
        copy: fn(&Arena<T>) -> Arena<T>,
    },
}

impl<T> From<Arena<T>> for ArenaPage<T> {
    fn from(arena: Arena<T>) -> Self {
        Self::Owned(arena)
    }
}

impl<T> Deref for ArenaPage<T> {
    type Target = Arena<T>;
    fn deref(&self) -> &Arena<T> {
        match self {
            Self::Owned(arena) => arena,
            Self::Shared { arena, .. } => arena,
        }
    }
}

impl<T: Default> DerefMut for ArenaPage<T> {
    fn deref_mut(&mut self) -> &mut Arena<T> {
        if let Self::Shared { arena, copy } = self {
            if Arc::strong_count(arena) > 1 {
                let detached = copy(arena);
                *self = Self::Owned(detached);
            } else {
                let Self::Shared { arena, .. } =
                    std::mem::replace(self, Self::Owned(Arena::default()))
                else {
                    unreachable!()
                };
                *self = Self::Owned(match Arc::try_unwrap(arena) {
                    Ok(arena) => arena,
                    Err(_) => unreachable!("unique arena page gained an owner"),
                });
            }
        }
        match self {
            Self::Owned(arena) => arena,
            Self::Shared { .. } => unreachable!(),
        }
    }
}

impl<T: Default + Clone> ArenaPage<T> {
    fn share(&mut self) {
        if matches!(self, Self::Owned(arena) if !arena.is_empty()) {
            let Self::Owned(arena) = std::mem::replace(self, Self::Owned(Arena::default())) else {
                unreachable!()
            };
            *self = Self::Shared {
                arena: Arc::new(arena),
                copy: Arena::clone,
            };
        }
    }
}

impl<T: Clone> Clone for ArenaPage<T> {
    fn clone(&self) -> Self {
        match self {
            Self::Owned(arena) => Self::Owned(arena.clone()),
            Self::Shared { arena, copy } => Self::Shared {
                arena: arena.clone(),
                copy: *copy,
            },
        }
    }
    fn clone_from(&mut self, other: &Self) {
        match (self, other) {
            (Self::Owned(destination), Self::Owned(source)) => destination.clone_from(source),
            (destination, source) => *destination = source.clone(),
        }
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for ArenaPage<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.deref().fmt(formatter)
    }
}

impl<T: PartialEq> PartialEq for ArenaPage<T> {
    fn eq(&self, other: &Self) -> bool {
        self.deref() == other.deref()
    }
}

impl<T: Eq> Eq for ArenaPage<T> {}

impl<T: Default + Clone + record_storage::Record + 'static> record_storage::Record
    for ArenaPage<T>
{
    fn write(
        &self,
        output: &mut record_storage::RecordWriter,
    ) -> Result<(), record_storage::RecordError> {
        match self {
            Self::Owned(arena) => {
                false.write(output)?;
                arena.write(output)
            }
            Self::Shared { arena, .. } => {
                true.write(output)?;
                arena.write(output)
            }
        }
    }
    fn read<Context>(
        input: &mut record_storage::RecordReader<'_, Context>,
    ) -> Result<Self, record_storage::RecordError> {
        if bool::read(input)? {
            Ok(Self::Shared {
                arena: Arc::read(input)?,
                copy: Arena::clone,
            })
        } else {
            Arena::read(input).map(Self::Owned)
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct PagedArenaFrontier<T> {
    generation: u32,
    next_arena_index: u32,
    marker: PhantomData<fn() -> T>,
}

impl<T> PagedArenaFrontier<T> {
    pub fn contains(&self, handle: Handle<T>) -> bool {
        handle.generation() > self.generation
            || (handle.generation() == self.generation
                && handle.arena_index() >= self.next_arena_index)
    }
}

impl<T> Clone for PagedArenaFrontier<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for PagedArenaFrontier<T> {}

impl<T> Default for PagedArenaFrontier<T> {
    fn default() -> Self {
        Self {
            generation: 1,
            next_arena_index: 1,
            marker: PhantomData,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct PagedArenaSet<T: Default> {
    // One epoch covers page membership, selected generation, and mutable row access.
    // Adding child epochs cannot distinguish these independent inputs, and scanning
    // every page for each query would make a retained index depend on page count.
    revision: RevisionStamp,
    first_generation: u32,
    first_page: ArenaPage<T>,
    additional_pages: Vec<(u32, ArenaPage<T>)>,
    page_positions: Vec<u32>, // One-based index in `additional_pages` at `generation - page_base`.
    page_base: u32,           // Generation of the first additional page, or zero when empty.
    current_generation: u32,
    active_count: usize,
    dummy: T,
}

impl<T: Default + Clone> Clone for PagedArenaSet<T> {
    fn clone(&self) -> Self {
        Self {
            revision: self.revision.copy(),
            first_generation: self.first_generation,
            first_page: self.first_page.clone(),
            additional_pages: self.additional_pages.clone(),
            page_positions: self.page_positions.clone(),
            page_base: self.page_base,
            current_generation: self.current_generation,
            active_count: self.active_count,
            dummy: self.dummy.clone(),
        }
    }

    fn clone_from(&mut self, source: &Self) {
        self.revision.invalidate();
        self.first_generation = source.first_generation;
        self.first_page.clone_from(&source.first_page);
        self.additional_pages.clone_from(&source.additional_pages);
        self.page_positions.clone_from(&source.page_positions);
        self.page_base = source.page_base;
        self.current_generation = source.current_generation;
        self.active_count = source.active_count;
        self.dummy.clone_from(&source.dummy);
        self.revision = source.revision.copy();
    }
}

impl<T: Default> PagedArenaSet<T> {
    /// Moves a set holding one never-rewritten page to `generation`, so a
    /// page produced in one forest can be adopted by another without copying
    /// its rows. `visit` moves the handles each row stores.
    pub fn relocate_single_page(
        &mut self,
        generation: u32,
        visit: impl FnMut(&mut T),
    ) -> Result<(), &'static str> {
        if !self.additional_pages.is_empty() {
            return Err("only a single-page arena set can move generation");
        }
        self.revision.invalidate();
        self.first_page
            .relocate_fresh_generation(generation, visit)?;
        self.first_generation = generation;
        self.current_generation = generation;
        Ok(())
    }

    pub fn share_pages(&mut self)
    where
        T: Clone,
    {
        self.first_page.share();
        for (_, page) in &mut self.additional_pages {
            page.share();
        }
    }

    pub fn equal_by(&self, other: &Self, mut equal: impl FnMut(&T, &T) -> bool) -> bool {
        self.first_generation == other.first_generation
            && self.current_generation == other.current_generation
            && self.active_count == other.active_count
            && equal(&self.dummy, &other.dummy)
            && self.first_page.equal_by(&other.first_page, &mut equal)
            && self.additional_pages.len() == other.additional_pages.len()
            && self
                .additional_pages
                .iter()
                .zip(&other.additional_pages)
                .all(|((left_gen, left_page), (right_gen, right_page))| {
                    left_gen == right_gen && left_page.equal_by(right_page, &mut equal)
                })
    }

    pub fn new() -> Self {
        Self::with_generation(1)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        let mut set = Self::new();
        set.reserve(capacity);
        set
    }

    pub fn with_generation(generation: u32) -> Self {
        assert!(
            generation != 0,
            "paged arena set generation must be nonzero"
        );
        Self {
            revision: RevisionStamp::unstamped(),
            first_generation: generation,
            first_page: Arena::with_generation(generation).into(),
            additional_pages: Vec::new(),
            page_positions: Vec::new(),
            page_base: 0,
            current_generation: generation,
            active_count: 0,
            dummy: T::default(),
        }
    }

    pub fn adopt(&mut self, other: Self) {
        self.revision.invalidate();
        let other_current_generation = other.current_generation;
        let other_active_count = other.active_count;
        self.insert_page(other.first_generation, other.first_page);
        for (generation, page) in other.additional_pages {
            self.insert_page(generation, page);
        }
        self.current_generation = self.current_generation.max(other_current_generation);
        self.active_count = self
            .active_count
            .checked_add(other_active_count)
            .expect("paged arena active count overflow");
    }

    pub fn take_additional_pages(&mut self) -> Self {
        self.revision.invalidate();
        let mut taken = Self::with_generation(self.first_generation);
        taken.additional_pages = std::mem::take(&mut self.additional_pages);
        taken.page_positions = std::mem::take(&mut self.page_positions);
        taken.page_base = std::mem::take(&mut self.page_base);
        taken.active_count = taken
            .additional_pages
            .iter()
            .map(|(_, page)| page.len())
            .sum();
        taken.current_generation = self.current_generation;
        self.current_generation = self.first_generation;
        self.active_count = self.first_page.len();
        taken
    }

    pub fn adopt_additional_pages(&mut self, other: Self) {
        self.revision.invalidate();
        self.current_generation = self.current_generation.max(other.current_generation);
        if other.additional_pages.is_empty() {
            return;
        }
        for (generation, page) in other.additional_pages {
            self.active_count += page.len();
            self.insert_page(generation, page);
        }
    }

    pub fn latest_generation(&self) -> u32 {
        self.additional_pages
            .last()
            .map_or(self.current_generation, |(generation, _)| {
                (*generation).max(self.current_generation)
            })
    }

    pub fn current_generation(&self) -> u32 {
        self.current_generation
    }

    pub fn take_pages_from(&mut self, generation: u32, restored_current: u32) -> Self {
        self.revision.invalidate();
        assert!(
            generation > self.first_generation,
            "split generation must exceed first generation"
        );
        let split = self
            .additional_pages
            .partition_point(|(page_generation, _)| *page_generation < generation);
        let pages = self.additional_pages.split_off(split);
        let mut taken = Self::with_generation(self.first_generation);
        taken.current_generation = self.current_generation.max(generation);
        for (page_generation, page) in pages {
            taken.active_count += page.len();
            taken.insert_page(page_generation, page);
        }
        self.active_count -= taken.active_count;
        self.index_page_positions();
        self.current_generation = restored_current;
        taken
    }

    pub fn begin_page(&mut self, generation: u32) {
        assert!(
            generation != 0,
            "paged arena set generation must be nonzero"
        );
        assert!(
            generation > self.first_generation,
            "paged arena extension must exceed first generation"
        );
        assert!(
            self.page_for_generation(generation).is_none(),
            "paged arena generation {generation} already exists"
        );
        self.revision.invalidate();
        self.current_generation = generation;
    }

    pub fn first_generation(&self) -> u32 {
        self.first_generation
    }

    pub fn reserve(&mut self, additional: usize) {
        self.current_mut().reserve(additional);
    }

    pub fn insert(&mut self, item: T) -> Handle<T> {
        self.append(item)
    }

    pub fn free(&mut self, handle: Handle<T>) -> bool {
        let freed = self.page_mut(handle).is_some_and(|page| page.free(handle));
        if freed {
            self.active_count -= 1;
        }
        freed
    }

    pub fn append(&mut self, item: T) -> Handle<T> {
        let handle = self.current_mut().append(item);
        self.active_count += 1;
        handle
    }

    pub fn append_to_span(&mut self, span: &mut HandleSpan<T>, item: T) -> Handle<T>
    where
        T: Clone,
    {
        if !span.is_empty() && span.start().generation() != self.current_generation {
            let retained = self
                .span(*span)
                .expect("paged arena span append requires a live range")
                .to_vec();
            *span = self.insert_many(retained);
        }
        let before = self.current().map_or(0, Arena::len);
        let handle = self.current_mut().append_to_span(span, item);
        self.active_count += self.current().map_or(0, Arena::len) - before;
        handle
    }

    pub fn insert_many(&mut self, items: impl IntoIterator<Item = T>) -> HandleSpan<T> {
        let before = self.current().map_or(0, Arena::len);
        let span = self.current_mut().insert_many(items);
        self.active_count += self.current().map_or(0, Arena::len) - before;
        span
    }

    pub fn try_insert_many_with<E>(
        &mut self,
        insert_items: impl FnOnce(&mut ArenaSpanInserter<'_, T>) -> Result<(), E>,
    ) -> Result<HandleSpan<T>, E> {
        let before = self.current().map_or(0, Arena::len);
        let span = self.current_mut().try_insert_many_with(insert_items)?;
        self.active_count += self.current().map_or(0, Arena::len) - before;
        Ok(span)
    }

    pub fn try_insert_many<E>(
        &mut self,
        items: impl IntoIterator<Item = Result<T, E>>,
    ) -> Result<HandleSpan<T>, E> {
        let before = self.current().map_or(0, Arena::len);
        let span = self.current_mut().try_insert_many(items)?;
        self.active_count += self.current().map_or(0, Arena::len) - before;
        Ok(span)
    }

    pub fn copy_span_pair(&mut self, first: HandleSpan<T>, second: HandleSpan<T>) -> HandleSpan<T>
    where
        T: Clone,
    {
        let values = self
            .span(first)
            .into_iter()
            .flatten()
            .chain(self.span(second).into_iter().flatten())
            .cloned()
            .collect::<Vec<_>>();
        self.insert_many(values)
    }

    /// Handle zero answers with the shared default; any other handle must
    /// resolve on its page (see `refuse_unresolved` in `arena.rs`).
    #[inline]
    #[track_caller]
    pub fn get(&self, handle: Handle<T>) -> &T {
        if !handle.is_valid() {
            return &self.dummy;
        }
        match self.page(handle) {
            Some(page) => page.get(handle),
            None => refuse_unresolved("get", handle.arena_index(), handle.generation()),
        }
    }

    #[track_caller]
    pub fn get_mut(&mut self, handle: Handle<T>) -> &mut T {
        self.revision.invalidate();
        match self.page_mut(handle) {
            Some(page) => page.get_mut(handle),
            None => refuse_unresolved("get_mut", handle.arena_index(), handle.generation()),
        }
    }

    pub fn is_valid(&self, handle: Handle<T>) -> bool {
        self.page(handle).is_some_and(|page| page.is_valid(handle))
    }

    pub fn span(&self, span: HandleSpan<T>) -> Option<&[T]> {
        if span.is_empty() {
            return Some(&[]);
        }
        self.page(span.start())?.span(span)
    }

    #[track_caller]
    pub fn span_or_empty(&self, span: HandleSpan<T>) -> &[T] {
        match self.span(span) {
            Some(items) => items,
            None => refuse_unresolved_span("span_or_empty", span),
        }
    }

    pub fn span_mut(&mut self, span: HandleSpan<T>) -> Option<&mut [T]> {
        if span.is_empty() {
            return Some(&mut []);
        }
        self.page_mut(span.start())?.span_mut(span)
    }

    #[track_caller]
    pub fn span_mut_or_empty(&mut self, span: HandleSpan<T>) -> &mut [T] {
        match self.span_mut(span) {
            Some(items) => items,
            None => refuse_unresolved_span("span_mut_or_empty", span),
        }
    }

    pub fn len(&self) -> usize {
        self.active_count
    }

    pub fn is_empty(&self) -> bool {
        self.active_count == 0
    }

    pub fn iter(&self) -> impl Iterator<Item = (Handle<T>, &T)> {
        std::iter::once(&self.first_page)
            .chain(self.additional_pages.iter().map(|(_, page)| page))
            .flat_map(|page| page.iter())
    }

    pub fn for_each_mut(&mut self, mut visit: impl FnMut(Handle<T>, &mut T)) {
        self.revision.invalidate();
        self.first_page.for_each_mut(&mut visit);
        for (_, page) in &mut self.additional_pages {
            page.for_each_mut(&mut visit);
        }
    }

    pub fn rewind_appended(&mut self, frontier: PagedArenaFrontier<T>) {
        assert_eq!(
            self.current_generation, frontier.generation,
            "append rewind cannot cross a published page"
        );
        let length = usize::try_from(frontier.next_arena_index - 1)
            .expect("arena frontier length exceeds usize");
        let removed = self
            .current()
            .map_or(0, Arena::len)
            .checked_sub(length)
            .expect("append rewind exceeds page");
        if removed != 0 {
            self.current_mut().rewind_appended(length);
            self.active_count -= removed;
        }
    }

    pub fn span_since(&self, frontier: PagedArenaFrontier<T>) -> HandleSpan<T> {
        let end = self.frontier();
        let start = if frontier.generation == end.generation {
            frontier.next_arena_index
        } else {
            1
        };
        let count = end.next_arena_index.saturating_sub(start);
        if count == 0 {
            HandleSpan::empty()
        } else {
            HandleSpan::from_parts(Handle::from_parts(start, end.generation), count)
        }
    }

    pub fn reset_retain_capacity(&mut self) {
        self.revision.invalidate();
        self.first_page.reset_retain_capacity();
        self.additional_pages.clear();
        self.page_positions.clear();
        self.page_base = 0;
        self.current_generation = self.first_generation;
        self.active_count = 0;
        self.dummy = T::default();
    }

    pub fn revision(&self) -> u64 {
        self.revision.get()
    }

    pub fn handle_of(&self, item: &T) -> Option<Handle<T>> {
        self.pages().find_map(|page| page.handle_of(item))
    }

    fn pages(&self) -> impl Iterator<Item = &Arena<T>> {
        std::iter::once(&*self.first_page)
            .chain(self.additional_pages.iter().map(|(_, page)| &**page))
    }

    pub fn page_count(&self) -> usize {
        1 + usize::try_from(self.current_generation - self.first_generation)
            .expect("paged arena page count overflow")
    }

    pub fn frontier(&self) -> PagedArenaFrontier<T> {
        let next_arena_index = self
            .current()
            .map_or(0, |page| page.storage_slice().len())
            .checked_add(1)
            .and_then(|index| u32::try_from(index).ok())
            .expect("paged arena frontier index overflow");
        PagedArenaFrontier {
            generation: self.current_generation,
            next_arena_index,
            marker: PhantomData,
        }
    }

    fn current(&self) -> Option<&Arena<T>> {
        self.page_for_generation(self.current_generation)
    }

    fn current_mut(&mut self) -> &mut Arena<T> {
        self.revision.invalidate();
        let generation = self.current_generation;
        if generation == self.first_generation {
            return &mut self.first_page;
        }
        let position = match self.page_position(generation) {
            Some(position) => position,
            None => self.insert_page(generation, Arena::with_generation(generation).into()),
        };
        &mut self.additional_pages[position].1
    }

    fn page(&self, handle: Handle<T>) -> Option<&Arena<T>> {
        self.page_for_generation(handle.generation())
    }

    fn page_mut(&mut self, handle: Handle<T>) -> Option<&mut Arena<T>> {
        self.page_for_generation_mut(handle.generation())
    }

    #[inline]
    fn page_for_generation(&self, generation: u32) -> Option<&Arena<T>> {
        if generation == self.first_generation {
            return Some(&self.first_page);
        }
        let position = self.page_position(generation)?;
        Some(&self.additional_pages[position].1)
    }

    fn page_for_generation_mut(&mut self, generation: u32) -> Option<&mut Arena<T>> {
        self.revision.invalidate();
        if generation == self.first_generation {
            return Some(&mut self.first_page);
        }
        let position = self.page_position(generation)?;
        Some(&mut self.additional_pages[position].1)
    }

    #[inline]
    fn page_position(&self, generation: u32) -> Option<usize> {
        let offset = generation.checked_sub(self.page_base)?;
        let position = *self.page_positions.get(usize::try_from(offset).ok()?)?;
        usize::try_from(position).ok()?.checked_sub(1)
    }

    fn record_page_position(&mut self, generation: u32, position: usize) {
        let offset = usize::try_from(generation - self.page_base)
            .expect("paged arena generation exceeds usize");
        if self.page_positions.len() <= offset {
            self.page_positions.resize(offset + 1, 0);
        }
        self.page_positions[offset] =
            u32::try_from(position + 1).expect("paged arena page roster overflow");
    }

    fn index_page_positions(&mut self) {
        self.page_positions.clear();
        self.page_base = self.additional_pages.first().map_or(0, |(base, _)| *base);
        for position in 0..self.additional_pages.len() {
            self.record_page_position(self.additional_pages[position].0, position);
        }
    }

    fn insert_page(&mut self, generation: u32, page: ArenaPage<T>) -> usize {
        self.revision.invalidate();
        assert!(
            generation > self.first_generation,
            "additional page generation must exceed first generation"
        );
        if self
            .additional_pages
            .last()
            .is_none_or(|(last_gen, _)| *last_gen < generation)
        {
            self.additional_pages.push((generation, page));
            let position = self.additional_pages.len() - 1;
            if position == 0 {
                self.page_positions.clear();
                self.page_base = generation;
            }
            self.record_page_position(generation, position);
            return position;
        }
        match self
            .additional_pages
            .binary_search_by_key(&generation, |(page_gen, _)| *page_gen)
        {
            Ok(_) => panic!("paged arena generation {generation} already exists"),
            Err(position) => {
                self.additional_pages.insert(position, (generation, page));
                self.index_page_positions();
                position
            }
        }
    }
}

impl<T: Default> Default for PagedArenaSet<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> record_storage::Record for PagedArenaFrontier<T> {
    fn write(
        &self,
        output: &mut record_storage::RecordWriter,
    ) -> Result<(), record_storage::RecordError> {
        self.generation.write(output)?;
        self.next_arena_index.write(output)
    }
    fn read<Context>(
        input: &mut record_storage::RecordReader<'_, Context>,
    ) -> Result<Self, record_storage::RecordError> {
        let generation = u32::read(input)?;
        let next_arena_index = u32::read(input)?;
        if generation == 0 || next_arena_index == 0 {
            return Err(input.error("paged arena frontier has an invalid coordinate"));
        }
        Ok(Self {
            generation,
            next_arena_index,
            marker: PhantomData,
        })
    }
}

impl<T: Default + Clone + record_storage::Record + 'static> record_storage::Record
    for PagedArenaSet<T>
{
    fn write(
        &self,
        output: &mut record_storage::RecordWriter,
    ) -> Result<(), record_storage::RecordError> {
        output.record(|output| {
            self.first_generation.write(output)?;
            self.first_page.write(output)?;
            self.additional_pages.write(output)?;
            self.current_generation.write(output)?;
            self.active_count.write(output)?;
            self.dummy.write(output)
        })
    }
    fn read<Context>(
        input: &mut record_storage::RecordReader<'_, Context>,
    ) -> Result<Self, record_storage::RecordError> {
        input.record(|input| {
            let mut result = Self {
                revision: RevisionStamp::unstamped(),
                first_generation: u32::read(input)?,
                first_page: ArenaPage::read(input)?,
                additional_pages: Vec::read(input)?,
                page_positions: Vec::new(),
                page_base: 0,
                current_generation: u32::read(input)?,
                active_count: usize::read(input)?,
                dummy: T::read(input)?,
            };
            if result.first_generation == 0
                || result.current_generation < result.first_generation
                || result.first_page.storage_view().fresh_generation != result.first_generation
            {
                return Err(input.error("paged arena has an invalid first or current page"));
            }
            let mut previous_generation = result.first_generation;
            for (generation, page) in &result.additional_pages {
                if *generation <= previous_generation {
                    return Err(
                        input.error("paged arena page generations are not strictly ordered")
                    );
                }
                if page.storage_view().fresh_generation != *generation {
                    return Err(input.error("paged arena slot disagrees with its page generation"));
                }
                previous_generation = *generation;
            }
            if result
                .pages()
                .try_fold(0usize, |sum, page| sum.checked_add(page.len()))
                != Some(result.active_count)
            {
                return Err(input.error("paged arena live row count disagrees with its pages"));
            }
            let span = match (
                result.additional_pages.first(),
                result.additional_pages.last(),
            ) {
                (Some((first, _)), Some((last, _))) => last - first + 1,
                _ => 0,
            };
            input.allocate(
                usize::try_from(span)
                    .ok()
                    .and_then(|span| span.checked_mul(std::mem::size_of::<u32>()))
                    .ok_or_else(|| input.error("paged arena directory exceeds usize"))?,
            )?;
            result.index_page_positions();
            Ok(result)
        })
    }
}

#[derive(record_storage::Record)]
pub struct OwnedInterner<T> {
    table: Mutex<T>,
}

impl<T: Default> Default for OwnedInterner<T> {
    fn default() -> Self {
        Self {
            table: Mutex::new(T::default()),
        }
    }
}

impl<T: Clone> Clone for OwnedInterner<T> {
    fn clone(&self) -> Self {
        Self {
            table: Mutex::new(
                self.table
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .clone(),
            ),
        }
    }
}

impl<T: PartialEq> PartialEq for OwnedInterner<T> {
    fn eq(&self, other: &Self) -> bool {
        if std::ptr::eq(self, other) {
            return true;
        }
        // Concurrent comparisons in opposite directions acquire both tables
        // in the same order. Neither table can change during the comparison.
        let (first, second) = if std::ptr::from_ref(self).addr() < std::ptr::from_ref(other).addr() {
            (self, other)
        } else {
            (other, self)
        };
        first.with(|left| second.with(|right| left == right))
    }
}

impl<T: Eq> Eq for OwnedInterner<T> {}

impl<T> std::fmt::Debug for OwnedInterner<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("OwnedInterner")
    }
}

impl<T> OwnedInterner<T> {
    pub fn with<R>(&self, run: impl FnOnce(&T) -> R) -> R {
        let table = self.table.lock().unwrap_or_else(|error| error.into_inner());
        run(&table)
    }

    pub fn with_mut<R>(&self, run: impl FnOnce(&mut T) -> R) -> R {
        let mut table = self.table.lock().unwrap_or_else(|error| error.into_inner());
        run(&mut table)
    }
}

pub struct OwnedTextInterner {
    prefixes: Vec<TextPrefix>,
    prefix_len: usize,
    local: Arc<TextInternerStorage>,
}

#[derive(Clone)]
struct TextPrefix {
    start: usize,
    end: usize,
    storage: Arc<TextInternerStorage>,
}

struct TextInternerStorage {
    texts: [OnceLock<Box<[OnceLock<Box<str>>]>>; TEXT_PAGES],
    indexes: [OnceLock<TextIndex>; INDEX_GENERATIONS],
    generation: AtomicUsize,
    minted: Mutex<usize>,
}

const FIRST_TEXT_PAGE: usize = 256;
const TEXT_PAGES: usize = 25;
const FIRST_INDEX_WORDS: usize = 1024;
const INDEX_GENERATIONS: usize = 24;

struct TextIndex {
    words: Box<[AtomicU64]>,
}

enum Probe {
    Found(usize),
    Vacant(usize),
}

impl Default for OwnedTextInterner {
    fn default() -> Self {
        Self {
            prefixes: Vec::new(),
            prefix_len: 0,
            local: Arc::new(TextInternerStorage::default()),
        }
    }
}

impl Default for TextInternerStorage {
    fn default() -> Self {
        Self {
            texts: [const { OnceLock::new() }; TEXT_PAGES],
            indexes: [const { OnceLock::new() }; INDEX_GENERATIONS],
            generation: AtomicUsize::new(0),
            minted: Mutex::new(0),
        }
    }
}

impl OwnedTextInterner {
    // A fork captures each prefix's visible length. The producer may append to
    // its storage later, but those slots never enter the fork's numbering.
    // New texts use a private storage, not a copy of the retained prefix.
    pub fn intern(&self, text: &str) -> usize {
        if let Some(slot) = self.prefix_slot(text) {
            return slot;
        }
        self.prefix_len + self.local.intern(text, u32::MAX as usize - self.prefix_len)
    }

    pub fn text(&self, slot: usize) -> Option<&str> {
        if slot >= u32::MAX as usize {
            return None;
        }
        if slot >= self.prefix_len {
            return self.local.text(slot - self.prefix_len);
        }
        let position = self.prefixes.partition_point(|prefix| prefix.end <= slot);
        let prefix = self.prefixes.get(position)?;
        prefix.storage.text(slot - prefix.start)
    }

    fn prefix_slot(&self, text: &str) -> Option<usize> {
        self.prefixes.iter().find_map(|prefix| {
            let slot = prefix.storage.slot(text)?;
            (slot < prefix.end - prefix.start).then_some(prefix.start + slot)
        })
    }

    pub fn slot(&self, text: &str) -> Option<usize> {
        self.prefix_slot(text)
            .or_else(|| self.local.slot(text).map(|slot| self.prefix_len + slot))
    }

    pub fn len(&self) -> usize {
        self.prefix_len + self.local.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn texts_in_order(&self) -> Vec<&str> {
        let length = self.len();
        (0..length)
            .map(|slot| self.text(slot).expect("a minted slot publishes its text"))
            .collect()
    }
}

impl TextInternerStorage {
    fn intern(&self, text: &str, capacity: usize) -> usize {
        let tag = text_tag(text);
        if let Probe::Found(slot) = self.current_index().probe(tag, text, self) {
            return slot;
        }
        let mut minted = self
            .minted
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let generation = self.generation.load(Ordering::Acquire);
        let index = self.index(generation);
        let vacant = match index.probe(tag, text, self) {
            Probe::Found(slot) => return slot,
            Probe::Vacant(position) => position,
        };
        let slot = *minted;
        assert!(slot < capacity, "text interner slot space exhausted");
        *minted += 1;
        let (page, offset) = text_position(slot);
        let published = self.texts[page].get_or_init(|| {
            (0..FIRST_TEXT_PAGE << page)
                .map(|_| OnceLock::new())
                .collect()
        })[offset]
            .set(text.into());
        debug_assert!(published.is_ok(), "a text slot is minted once");
        let word = index_word(tag, slot);
        if *minted * 2 > index.words.len() {
            let grown = TextIndex::with_words(index.words.len() * 2);
            for word in index.words.iter() {
                let word = word.load(Ordering::Relaxed);
                if word != 0 {
                    grown.insert(word);
                }
            }
            grown.insert(word);
            assert!(
                self.indexes[generation + 1].set(grown).is_ok(),
                "only minting grows the index"
            );
            self.generation.store(generation + 1, Ordering::Release);
        } else {
            index.words[vacant].store(word, Ordering::Release);
        }
        slot
    }

    pub fn text(&self, slot: usize) -> Option<&str> {
        let (page, offset) = text_position(slot);
        self.texts
            .get(page)?
            .get()?
            .get(offset)?
            .get()
            .map(AsRef::as_ref)
    }

    pub fn slot(&self, text: &str) -> Option<usize> {
        let tag = text_tag(text);
        loop {
            let generation = self.generation.load(Ordering::Acquire);
            if let Probe::Found(slot) = self.index(generation).probe(tag, text, self) {
                return Some(slot);
            }
            if self.generation.load(Ordering::Acquire) == generation {
                return None;
            }
        }
    }

    pub fn len(&self) -> usize {
        *self
            .minted
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }

    fn current_index(&self) -> &TextIndex {
        self.index(self.generation.load(Ordering::Acquire))
    }

    fn index(&self, generation: usize) -> &TextIndex {
        if generation == 0 {
            self.indexes[0].get_or_init(|| TextIndex::with_words(FIRST_INDEX_WORDS))
        } else {
            self.indexes[generation]
                .get()
                .expect("the current index generation is published before it is named")
        }
    }
}

impl TextIndex {
    fn with_words(count: usize) -> Self {
        Self {
            words: (0..count).map(|_| AtomicU64::new(0)).collect(),
        }
    }

    fn probe(&self, tag: u32, text: &str, interner: &TextInternerStorage) -> Probe {
        let mask = self.words.len() - 1;
        let mut position = index_start(tag, mask);
        loop {
            let word = self.words[position].load(Ordering::Acquire);
            if word == 0 {
                return Probe::Vacant(position);
            }
            let slot = (word as u32 - 1) as usize;
            if (word >> 32) as u32 == tag && interner.text(slot) == Some(text) {
                return Probe::Found(slot);
            }
            position = (position + 1) & mask;
        }
    }

    fn insert(&self, word: u64) {
        let mask = self.words.len() - 1;
        let mut position = index_start((word >> 32) as u32, mask);
        while self.words[position].load(Ordering::Relaxed) != 0 {
            position = (position + 1) & mask;
        }
        self.words[position].store(word, Ordering::Relaxed);
    }
}

fn index_word(tag: u32, slot: usize) -> u64 {
    (u64::from(tag) << 32) | (slot as u64 + 1)
}

fn index_start(tag: u32, mask: usize) -> usize {
    (u64::from(tag).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 32) as usize & mask
}

fn text_position(slot: usize) -> (usize, usize) {
    let biased = slot + FIRST_TEXT_PAGE;
    let page = (biased.ilog2() - FIRST_TEXT_PAGE.ilog2()) as usize;
    (page, biased - (FIRST_TEXT_PAGE << page))
}

fn text_tag(text: &str) -> u32 {
    const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;
    let bytes = text.as_bytes();
    let mut hash = bytes.len() as u64;
    let (words, remainder) = bytes.as_chunks::<8>();
    for word in words {
        hash = (hash.rotate_left(5) ^ u64::from_le_bytes(*word)).wrapping_mul(SEED);
    }
    let mut tail = [0u8; 8];
    tail[..remainder.len()].copy_from_slice(remainder);
    hash = (hash.rotate_left(5) ^ u64::from_le_bytes(tail)).wrapping_mul(SEED);
    ((hash >> 32) ^ hash) as u32
}

impl Clone for OwnedTextInterner {
    fn clone(&self) -> Self {
        let local_len = self.local.len();
        let mut prefixes = Vec::with_capacity(self.prefixes.len() + usize::from(local_len != 0));
        prefixes.extend(self.prefixes.iter().cloned());
        let prefix_len = self.prefix_len + local_len;
        if local_len != 0 {
            prefixes.push(TextPrefix {
                start: self.prefix_len,
                end: prefix_len,
                storage: Arc::clone(&self.local),
            });
        }
        Self {
            prefixes,
            prefix_len,
            local: Arc::new(TextInternerStorage::default()),
        }
    }
}

impl PartialEq for OwnedTextInterner {
    fn eq(&self, other: &Self) -> bool {
        if std::ptr::eq(self, other) {
            return true;
        }
        let length = self.len();
        length == other.len() && (0..length).all(|slot| self.text(slot) == other.text(slot))
    }
}

impl Eq for OwnedTextInterner {}

impl std::fmt::Debug for OwnedTextInterner {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("OwnedTextInterner")
    }
}

fn write_texts(
    output: &mut record_storage::RecordWriter,
    texts: &[&str],
) -> Result<(), record_storage::RecordError> {
    output.record(|output| {
        output.sequence(texts.len())?;
        for text in texts {
            output.sequence(text.len())?;
            output.extend(text.as_bytes())?;
        }
        Ok(())
    })
}

impl OwnedTextInterner {
    /*
        Each minted slot's place when the texts are sorted. Slots minted by
        parallel passes race for their numbers; these places depend only on
        the texts.
    */
    pub fn sorted_numbering(&self) -> Vec<u32> {
        let texts = self.texts_in_order();
        let mut order: Vec<usize> = (0..texts.len()).collect();
        order.sort_unstable_by(|left, right| texts[*left].cmp(texts[*right]));
        let mut numbers = vec![0; texts.len()];
        for (place, slot) in order.into_iter().enumerate() {
            numbers[slot] = place as u32;
        }
        numbers
    }

    /*
        Writes the texts as the ordinary record does, each at `numbers[slot]`.
        A reader rebuilds the interner in that order, so reading back gives
        every text the number written for it. `numbers` must be a numbering
        of exactly these slots, such as `sorted_numbering`.
    */
    pub fn write_renumbered(
        &self,
        output: &mut record_storage::RecordWriter,
        numbers: &[u32],
    ) -> Result<(), record_storage::RecordError> {
        let texts = self.texts_in_order();
        if numbers.len() != texts.len() {
            return Err(output.error("a renumbering of another set of texts"));
        }
        let mut placed: Vec<Option<&str>> = vec![None; texts.len()];
        for (slot, text) in texts.into_iter().enumerate() {
            let Some(place) = placed.get_mut(numbers[slot] as usize) else {
                return Err(output.error("a renumbering past its texts"));
            };
            if place.replace(text).is_some() {
                return Err(output.error("a renumbering gives two texts one number"));
            }
        }
        let placed: Vec<&str> = placed.into_iter().map(Option::unwrap_or_default).collect();
        write_texts(output, &placed)
    }
}

impl record_storage::Record for OwnedTextInterner {
    fn write(
        &self,
        output: &mut record_storage::RecordWriter,
    ) -> Result<(), record_storage::RecordError> {
        write_texts(output, &self.texts_in_order())
    }

    fn read<Context>(
        input: &mut record_storage::RecordReader<'_, Context>,
    ) -> Result<Self, record_storage::RecordError> {
        input.record(|input| {
            let count = input.sequence::<()>()?;
            if count > u32::MAX as usize {
                return Err(input.error("text interner slot space exhausted"));
            }
            // Decode directly from the input bytes. Charge the retained core,
            // geometric text pages and every retained index generation before
            // allocating them; each spelling is charged by its byte sequence.
            let mut allocation =
                record_storage::shared_allocation_bytes(std::alloc::Layout::new::<
                    TextInternerStorage,
                >())
                .ok_or_else(|| input.error("text storage allocation overflow"))?;
            if count != 0 {
                let (last_page, _) = text_position(count - 1);
                for page in 0..=last_page {
                    allocation = allocation
                        .checked_add(
                            (FIRST_TEXT_PAGE << page)
                                .checked_mul(std::mem::size_of::<OnceLock<Box<str>>>())
                                .ok_or_else(|| input.error("text page allocation overflow"))?,
                        )
                        .ok_or_else(|| input.error("text storage allocation overflow"))?;
                }
                let mut words = FIRST_INDEX_WORDS;
                loop {
                    allocation = allocation
                        .checked_add(
                            words
                                .checked_mul(std::mem::size_of::<AtomicU64>())
                                .ok_or_else(|| input.error("text index allocation overflow"))?,
                        )
                        .ok_or_else(|| input.error("text storage allocation overflow"))?;
                    if count <= words / 2 {
                        break;
                    }
                    words = words
                        .checked_mul(2)
                        .ok_or_else(|| input.error("text index allocation overflow"))?;
                }
            }
            input.allocate(allocation)?;
            let interner = Self::default();
            for slot in 0..count {
                let length = input.sequence::<u8>()?;
                let text = std::str::from_utf8(input.take(length)?)
                    .map_err(|_| input.error("invalid UTF-8"))?;
                if interner.intern(text) != slot {
                    return Err(input.error("duplicate interned text"));
                }
            }
            Ok(interner)
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{
        FIRST_INDEX_WORDS, FIRST_TEXT_PAGE, OwnedInterner, OwnedTextInterner, PagedArenaSet,
    };
    use crate::Handle;

    #[test]
    fn checkpoint_shares_payloads_and_detaches_only_edited_pages() {
        let mut parent = PagedArenaSet::<String>::new();
        let first = parent.insert_many(["original".to_owned(), "neighbor".to_owned()]);
        parent.begin_page(2);
        let second = parent.append("second page".to_owned());
        parent.share_pages();
        let mut child = parent.clone();
        assert_eq!(parent, child);
        assert!(std::ptr::eq(
            parent.get(first.start()),
            child.get(first.start())
        ));
        assert!(std::ptr::eq(parent.get(second), child.get(second)));
        child.begin_page(3);
        let extension = child.append("child only".to_owned());
        assert!(std::ptr::eq(
            parent.get(first.start()),
            child.get(first.start())
        ));
        child.span_mut(first).unwrap()[0].push_str(" changed");
        assert_eq!(parent.get(first.start()), "original");
        assert_eq!(child.get(first.start()), "original changed");
        assert!(std::ptr::eq(parent.get(second), child.get(second)));
        assert!(!parent.is_valid(extension));
        assert!(child.free(second));
        assert_eq!(parent.get(second), "second page");
        drop(parent);
        assert_eq!(child.get(extension), "child only");
    }

    #[test]
    fn workers_hand_back_only_pages_from_their_split_generation() {
        let mut baseline = PagedArenaSet::<String>::new();
        let prefix = baseline.append("prefix".to_owned());
        baseline.begin_page(5);
        let earlier = baseline.append("earlier page".to_owned());
        baseline.share_pages();
        let split = baseline.latest_generation() + 1;
        assert_eq!(split, 6);
        let restored = baseline.current_generation();

        let mut outputs = Vec::new();
        let mut minted = Vec::new();
        for worker in 0..2u32 {
            let mut fork = baseline.clone();
            fork.begin_page(split + worker);
            let handle = fork.append(format!("worker {worker}"));
            assert_eq!(fork.get(prefix), "prefix");
            assert_eq!(fork.get(earlier), "earlier page");
            let pages = fork.take_pages_from(split, restored);
            assert_eq!(fork, baseline);
            minted.push(handle);
            outputs.push(pages);
        }
        let mut owner = baseline.clone();
        for pages in outputs {
            owner.adopt_additional_pages(pages);
        }
        assert_eq!(owner.get(prefix), "prefix");
        assert_eq!(owner.get(earlier), "earlier page");
        assert_eq!(owner.get(minted[0]), "worker 0");
        assert_eq!(owner.get(minted[1]), "worker 1");
        assert_eq!(
            owner
                .iter()
                .map(|(_, row)| row.as_str())
                .collect::<Vec<_>>(),
            ["prefix", "earlier page", "worker 0", "worker 1"]
        );
        assert_eq!(owner.len(), 4);
    }

    #[test]
    fn late_page_directory_spans_only_additional_pages() {
        let mut set = PagedArenaSet::<String>::new();
        set.begin_page(90_000);
        let late = set.append("late".to_owned());
        assert_eq!(set.page_positions.len(), 1);
        set.begin_page(90_002);
        let later = set.append("later".to_owned());
        assert_eq!(set.page_positions.len(), 3);
        set.begin_page(80_000);
        let earlier = set.append("earlier".to_owned());
        assert_eq!(set.page_positions.len(), 10_003);
        assert_eq!(set.get(late), "late");
        assert_eq!(set.get(later), "later");
        assert_eq!(set.get(earlier), "earlier");
        assert!(!set.is_valid(Handle::<String>::from_parts(1, 90_001)));
        let taken = set.take_additional_pages();
        assert!(set.page_positions.is_empty());
        set.begin_page(95_000);
        let next = set.append("next".to_owned());
        assert_eq!(set.page_positions.len(), 1);
        assert_eq!(set.get(next), "next");
        set.adopt_additional_pages(taken);
        assert_eq!(set.get(late), "late");
        assert_eq!(set.get(earlier), "earlier");
        assert_eq!(set.get(next), "next");
    }

    #[test]
    fn high_generation_page_has_no_empty_prefix() {
        let mut page = PagedArenaSet::<String>::with_generation(100_000);
        let value = page.append("value".to_owned());

        assert!(page.additional_pages.is_empty());
        assert_eq!(page.first_generation, 100_000);
        assert_eq!(page.get(value), "value");
    }

    #[test]
    fn adopts_dense_pages_without_remapping_handles() {
        let mut first = PagedArenaSet::<String>::with_generation(1);
        let alpha = first.append("alpha".to_owned());
        let mut second = PagedArenaSet::<String>::with_generation(2);
        let beta = second.append("beta".to_owned());
        first.adopt(second);

        assert_eq!(first.get(alpha), "alpha");
        assert_eq!(first.get(beta), "beta");
        assert_eq!(first.len(), 2);
        assert_eq!(first.page_count(), 2);
    }

    #[test]
    fn relocating_a_span_moves_only_that_small_roster_to_the_extension_page() {
        let mut first = PagedArenaSet::<String>::with_generation(1);
        let mut names = first.insert_many(["alpha".to_owned(), "beta".to_owned()]);
        first.begin_page(2);
        first.append_to_span(&mut names, "gamma".to_owned());

        assert_eq!(names.start().generation(), 2);
        assert_eq!(first.span_or_empty(names), ["alpha", "beta", "gamma"]);
    }

    #[test]
    fn span_since_names_rows_appended_after_a_frontier_on_any_page() {
        let mut set = PagedArenaSet::<String>::new();
        set.append("before".to_owned());
        let same_page = set.frontier();
        set.append("first".to_owned());
        set.append("second".to_owned());
        assert_eq!(
            set.span_or_empty(set.span_since(same_page)),
            ["first", "second"]
        );
        assert!(set.span_since(set.frontier()).is_empty());

        let earlier_page = set.frontier();
        set.begin_page(2);
        set.append("later".to_owned());
        assert_eq!(set.span_or_empty(set.span_since(earlier_page)), ["later"]);
    }

    #[test]
    fn revision_grows_with_every_mutation_and_page() {
        let mut set = PagedArenaSet::<String>::new();
        let empty = set.revision();
        let handle = set.append("value".to_owned());
        let appended = set.revision();
        set.get_mut(handle).push('!');
        let rewritten = set.revision();
        set.begin_page(2);
        let paged = set.revision();
        assert!(empty < appended && appended < rewritten && rewritten < paged);
    }

    #[test]
    fn revision_distinguishes_page_contents_from_selected_generation() {
        let mut original = PagedArenaSet::<u64>::new();
        original.append(10);
        let original_revision = original.revision();
        let mut replacement = PagedArenaSet::<u64>::new();
        replacement.append(20);
        let replacement_revision = replacement.revision();

        // Selecting a later, still empty page must not disguise different contents
        // as the epoch of replacement storage retained by the same parent index.
        let generation = u32::try_from(replacement_revision - original_revision + 1)
            .expect("this test's two queries fit one generation gap");
        original.begin_page(generation);
        let index = crate::OwnedIndex::<Vec<u64>>::default();
        let query = |rows: &PagedArenaSet<u64>| {
            index.with(
                rows.revision(),
                || rows.iter().map(|(_, value)| *value).collect(),
                Clone::clone,
            )
        };
        assert_eq!(query(&replacement), [20]);
        assert_eq!(query(&original), [10]);
    }

    #[test]
    fn revision_tracks_paged_mutation_and_transfer_routes() {
        let mut rows = PagedArenaSet::<u64>::new();
        let first = rows.insert_many([10, 20]);
        let mut previous = rows.revision();
        rows.share_pages();
        let mut copy = rows.clone();
        assert_eq!(rows.revision(), previous);
        assert_eq!(copy.revision(), previous);
        let mut changed = |rows: &PagedArenaSet<u64>| {
            let current = rows.revision();
            assert_ne!(current, previous);
            assert_eq!(rows.revision(), current);
            previous = current;
        };
        *rows.get_mut(first.start()) += 1;
        changed(&rows);
        rows.span_mut(first).expect("first span")[1] += 1;
        changed(&rows);
        rows.for_each_mut(|_, value| *value += 1);
        changed(&rows);
        let frontier = rows.frontier();
        rows.append(30);
        changed(&rows);
        rows.rewind_appended(frontier);
        changed(&rows);
        rows.begin_page(2);
        changed(&rows);
        let second = rows.append(40);
        changed(&rows);
        rows.span_mut_or_empty(crate::HandleSpan::from_parts(second, 1))[0] += 1;
        changed(&rows);
        assert!(rows.free(second));
        changed(&rows);
        let taken = rows.take_additional_pages();
        changed(&rows);
        rows.adopt_additional_pages(taken);
        changed(&rows);
        let taken = rows.take_pages_from(2, 1);
        changed(&rows);
        rows.adopt_additional_pages(taken);
        changed(&rows);
        copy.clone_from(&rows);
        assert_eq!(copy.revision(), rows.revision());
        assert_eq!(copy, rows);
        rows.reset_retain_capacity();
        changed(&rows);
        let mut supplied = PagedArenaSet::with_generation(3);
        supplied.append(50);
        rows.adopt(supplied);
        changed(&rows);
        let mut moved = PagedArenaSet::new();
        moved.append(60);
        let before_relocation = moved.revision();
        moved
            .relocate_single_page(4, |_| {})
            .expect("fresh single page");
        assert_ne!(moved.revision(), before_relocation);
    }

    #[test]
    fn relocating_a_stale_tail_span_counts_every_copied_row() {
        let mut set = PagedArenaSet::<String>::new();
        let mut stale = set.insert_many(["alpha".to_owned(), "beta".to_owned()]);
        set.append("tail".to_owned());
        set.append_to_span(&mut stale, "gamma".to_owned());
        assert_eq!(set.span_or_empty(stale), ["alpha", "beta", "gamma"]);
        assert_eq!(set.len(), set.iter().count());
    }

    #[test]
    fn reset_returns_to_an_empty_first_page() {
        let mut set = PagedArenaSet::<String>::new();
        set.append("alpha".to_owned());
        set.begin_page(2);
        set.append("beta".to_owned());
        set.reset_retain_capacity();
        assert!(set.is_empty());
        assert_eq!(set.page_count(), 1);
        let handle = set.append("gamma".to_owned());
        assert_eq!((handle.generation(), handle.arena_index()), (1, 1));
    }

    #[test]
    fn additional_pages_store_high_generations_without_empty_prefix() {
        let mut worker = PagedArenaSet::<u32>::new();
        worker.begin_page(4_000);
        let high_handle = worker.append(4_000);
        let taken = worker.take_additional_pages();
        assert_eq!(taken.additional_pages.len(), 1);
        assert_eq!(*taken.get(high_handle), 4_000);

        let mut merged = PagedArenaSet::<u32>::new();
        let mut handles = Vec::new();
        for generation in (2..=25).rev() {
            let mut single = PagedArenaSet::<u32>::new();
            single.begin_page(generation);
            handles.push((generation, single.append(generation * 10)));
            merged.adopt_additional_pages(single.take_additional_pages());
        }
        merged.adopt_additional_pages(taken);
        assert_eq!(merged.additional_pages.len(), 25);
        assert_eq!(merged.page_count(), 4_000);
        assert_eq!(*merged.get(high_handle), 4_000);
        for (generation, handle) in handles {
            assert_eq!(handle.generation(), generation);
            assert_eq!(*merged.get(handle), generation * 10);
        }
        assert!(!merged.is_valid(Handle::from_parts(1, 26)));
        assert!(!merged.is_valid(Handle::from_parts(1, 3_999)));
        assert!(!merged.is_valid(Handle::from_parts(1, 4_001)));
    }

    #[test]
    fn retained_sparse_pages_preserve_holes_and_a_lazy_append_frontier() {
        let mut original = PagedArenaSet::<String>::with_generation(7);
        let first = original.append("first".into());
        original.begin_page(4_000);
        let retired = original.append("retired".into());
        let high = original.append("high".into());
        assert!(original.free(retired));
        original.share_pages();
        original.begin_page(12);
        let limits = record_storage::RecordLimits::default();
        let bytes = record_storage::encode(&original, limits).unwrap();
        let mut restored: PagedArenaSet<String> = record_storage::decode(&bytes, limits).unwrap();

        assert_eq!(restored, original);
        assert!(!restored.is_valid(retired));
        assert_eq!(restored.get(first), "first");
        assert_eq!(restored.get(high), "high");
        let appended = restored.append("later".into());
        assert_eq!((appended.generation(), appended.arena_index()), (12, 1));
        assert_eq!(restored.get(appended), "later");
        assert_eq!(restored.len(), 3);
        assert_eq!(original.len(), 2);
        assert_eq!(restored.get(high), "high");
    }

    #[test]
    fn retained_pages_refuse_inconsistent_generation_and_live_row_metadata() {
        let mut original = PagedArenaSet::<u32>::with_generation(7);
        original.append(10);
        original.begin_page(12);
        original.append(20);
        original.begin_page(4_000);
        original.append(30);
        let limits = record_storage::RecordLimits::default();
        let mut malformed = Vec::new();
        let mut duplicate = original.clone();
        duplicate.additional_pages[1].0 = 12;
        malformed.push(duplicate);
        let mut unordered = original.clone();
        unordered.additional_pages.swap(0, 1);
        malformed.push(unordered);
        let mut mismatched_page = original.clone();
        mismatched_page.additional_pages[1].0 = 4_001;
        malformed.push(mismatched_page);
        let mut wrong_count = original.clone();
        wrong_count.active_count += 1;
        malformed.push(wrong_count);
        let mut invalid_frontier = original;
        invalid_frontier.current_generation = 0;
        malformed.push(invalid_frontier);
        for value in malformed {
            let bytes = record_storage::encode(&value, limits).unwrap();
            assert!(record_storage::decode::<PagedArenaSet<u32>>(&bytes, limits).is_err());
        }
    }

    #[test]
    #[should_panic(expected = "arena get received an unresolved handle")]
    fn paged_set_get_refuses_a_handle_from_an_unknown_page() {
        let mut set = PagedArenaSet::<u32>::new();
        let value = set.insert(1);
        let _ = set.get(Handle::from_parts(
            value.arena_index(),
            value.generation() + 7,
        ));
    }

    #[test]
    #[should_panic(expected = "arena get_mut received an unresolved handle")]
    fn paged_set_get_mut_refuses_handle_zero() {
        let mut set = PagedArenaSet::<u32>::new();
        set.insert(1);
        *set.get_mut(Handle::invalid()) = 5;
    }

    #[test]
    fn interned_keys_survive_clone_and_later_interning() {
        let interner = OwnedInterner::<Vec<&str>>::default();
        let first = interner.with_mut(|table| {
            table.push("writer");
            table.len() - 1
        });
        let copy = interner.clone();
        interner.with_mut(|table| table.push("record"));
        assert_eq!(interner.with(|table| table[first]), "writer");
        assert_eq!(copy.with(|table| table[first]), "writer");
    }

    #[test]
    fn interner_equality_compares_the_values_at_each_live_slot() {
        let left = OwnedInterner::<Vec<u32>>::default();
        left.with_mut(|rows| rows.extend([5, 8]));
        let right = left.clone();
        assert_eq!(left, right);
        right.with_mut(|rows| rows[1] = 9);
        assert_ne!(left, right);

        let texts = OwnedTextInterner::default();
        texts.intern("alpha");
        let fork = texts.clone();
        assert_eq!(texts, fork);
        assert_eq!(texts.intern("beta"), fork.intern("other"));
        assert_ne!(texts, fork);
        let reordered = OwnedTextInterner::default();
        reordered.intern("beta");
        reordered.intern("alpha");
        assert_ne!(texts, reordered);
    }

    #[test]
    fn parallel_text_interning_mints_each_key_once() {
        let interner = OwnedTextInterner::default();
        let keys = (0..(FIRST_TEXT_PAGE + FIRST_INDEX_WORDS) * 4)
            .map(|index| format!("key{index}"))
            .collect::<Vec<_>>();
        let seen = std::thread::scope(|scope| {
            let workers = (0..8)
                .map(|worker| {
                    let (interner, keys) = (&interner, &keys);
                    scope.spawn(move || {
                        let mut order = keys.iter().collect::<Vec<_>>();
                        order.rotate_left(worker * 97);
                        order
                            .into_iter()
                            .map(|key| (key.clone(), interner.intern(key)))
                            .collect::<std::collections::BTreeMap<_, _>>()
                    })
                })
                .collect::<Vec<_>>();
            workers
                .into_iter()
                .map(|worker| worker.join().unwrap())
                .collect::<Vec<_>>()
        });
        assert!(seen.windows(2).all(|pair| pair[0] == pair[1]));
        assert_eq!(interner.len(), keys.len());
        let copy = interner.clone();
        let decoded: OwnedTextInterner = record_storage::decode(
            &record_storage::encode(&interner, Default::default()).unwrap(),
            Default::default(),
        )
        .unwrap();
        for (key, slot) in &seen[0] {
            assert_eq!(interner.text(*slot), Some(key.as_str()));
            assert_eq!(interner.slot(key), Some(*slot));
            assert_eq!(copy.text(*slot), Some(key.as_str()));
            assert_eq!(decoded.slot(key), Some(*slot));
        }
        assert_eq!(interner.slot("never interned"), None);
        assert_eq!(interner.text(keys.len()), None);
    }

    #[test]
    fn text_forks_share_prefixes_but_number_and_append_independently() {
        let parent = OwnedTextInterner::default();
        for ordinal in 0..(FIRST_TEXT_PAGE * 3) {
            assert_eq!(parent.intern(&format!("prefix-{ordinal}")), ordinal);
        }
        let first = parent.clone();
        assert!(first.local.indexes[0].get().is_none());
        assert!(Arc::ptr_eq(&first.prefixes[0].storage, &parent.local));
        let prefix_text = first.text(0).unwrap().as_ptr();
        let prefix_index = first.prefixes[0].storage.current_index().words.as_ptr();
        let boundary = first.len();
        assert_eq!(parent.intern("parent-only"), boundary);
        assert_eq!(first.slot("parent-only"), None);
        assert_eq!(first.text(boundary), None);
        assert_eq!(first.intern("child-only"), boundary);
        // A key added later in the shared ancestor must not mask a local key.
        assert_eq!(parent.intern("child-only"), boundary + 1);
        assert_eq!(first.intern("child-only"), boundary);
        assert_eq!(parent.slot("child-only"), Some(boundary + 1));
        assert_eq!(first.text(0).unwrap().as_ptr(), prefix_text);
        assert_eq!(
            first.prefixes[0].storage.current_index().words.as_ptr(),
            prefix_index
        );
        assert_eq!(parent.slot("child-only"), Some(boundary + 1));

        let mut descendant = first;
        for generation in 0..24 {
            let child = descendant.clone();
            let boundary = child.len();
            let hidden = format!("ancestor-{generation}");
            descendant.intern(&hidden);
            assert_eq!(child.slot(&hidden), None);
            assert_eq!(child.text(boundary), None);
            assert_eq!(child.intern(&format!("descendant-{generation}")), boundary);
            assert_eq!(child.text(0).unwrap().as_ptr(), prefix_text);
            descendant = child;
        }
        drop(parent);
        assert_eq!(descendant.text(0), Some("prefix-0"));
        assert_eq!(descendant.text(usize::MAX), None);
        let bytes = record_storage::encode(&descendant, Default::default()).unwrap();
        let decoded: OwnedTextInterner =
            record_storage::decode(&bytes, Default::default()).unwrap();
        assert_eq!(decoded.texts_in_order(), descendant.texts_in_order());
        assert_eq!(
            record_storage::encode(&decoded, Default::default()).unwrap(),
            bytes
        );
    }

    #[test]
    fn text_decoder_charges_storage_and_refuses_duplicate_or_invalid_text() {
        let mut limits = record_storage::RecordLimits::default();
        let interner = OwnedTextInterner::default();
        interner.intern("owned");
        let bytes = record_storage::encode(&interner, limits).unwrap();
        assert_eq!(
            bytes,
            record_storage::encode(&vec!["owned".to_owned()], limits).unwrap()
        );
        let required = record_storage::shared_allocation_bytes(std::alloc::Layout::new::<
            super::TextInternerStorage,
        >())
        .unwrap()
            + FIRST_TEXT_PAGE * std::mem::size_of::<std::sync::OnceLock<Box<str>>>()
            + FIRST_INDEX_WORDS * std::mem::size_of::<u64>()
            + "owned".len();
        limits.allocation = required;
        limits.depth = 1;
        assert!(record_storage::decode::<OwnedTextInterner>(&bytes, limits).is_ok());
        limits.depth = 0;
        assert!(record_storage::decode::<OwnedTextInterner>(&bytes, limits).is_err());
        limits.depth = 1;
        limits.allocation -= 1;
        assert!(record_storage::decode::<OwnedTextInterner>(&bytes, limits).is_err());
        limits.allocation = FIRST_INDEX_WORDS * std::mem::size_of::<u64>();
        assert!(record_storage::decode::<OwnedTextInterner>(&bytes, limits).is_err());
        let duplicate = record_storage::encode(
            &vec!["same".to_owned(), "same".to_owned()],
            Default::default(),
        )
        .unwrap();
        let error = record_storage::decode::<OwnedTextInterner>(&duplicate, Default::default())
            .unwrap_err();
        assert!(error.to_string().contains("duplicate interned text"));
        let mut invalid = bytes;
        *invalid.last_mut().unwrap() = 0xff;
        let error = record_storage::decode::<OwnedTextInterner>(&invalid, Default::default())
            .unwrap_err();
        assert!(error.to_string().contains("invalid UTF-8"));
    }

    #[test]
    fn text_capacity_refusal_leaves_the_storage_unchanged() {
        let storage = super::TextInternerStorage::default();
        assert_eq!(storage.intern("first", 1), 0);
        assert!(std::panic::catch_unwind(|| storage.intern("second", 1)).is_err());
        assert_eq!(storage.len(), 1);
        assert_eq!(storage.slot("second"), None);
        assert_eq!(storage.intern("first", 1), 0);
        assert_eq!(storage.intern("second", 2), 1);
    }

    #[test]
    fn parallel_text_interning_in_a_fork_keeps_one_private_number_per_key() {
        let parent = OwnedTextInterner::default();
        parent.intern("prefix");
        let child = parent.clone();
        parent.intern("hidden");
        std::thread::scope(|scope| {
            for _ in 0..8 {
                let child = &child;
                scope.spawn(move || {
                    for ordinal in 0..300 {
                        let text = format!("new-{ordinal}");
                        let slot = child.intern(&text);
                        assert_eq!(child.slot(&text), Some(slot));
                        assert_eq!(child.text(slot), Some(text.as_str()));
                        assert_eq!(child.intern("prefix"), 0);
                        assert_eq!(child.slot("hidden"), None);
                    }
                });
            }
        });
        assert_eq!(child.len(), 301);
        assert_eq!(parent.len(), 2);
        assert_eq!(parent.slot("new-0"), None);
    }
}
