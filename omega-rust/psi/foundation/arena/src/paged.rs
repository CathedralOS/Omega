/*
We implement the multi-page and concurrent arena structures (`PagedArenaSet`,
`ConcurrentPagedArena`, `GenerationalPagedArena`, `OwnedInterner`, and
`OwnedTextInterner`) used across parallel Psi compilation stages.

`PagedArenaSet` groups multiple dense `Arena` pages behind one `Handle` surface where
the handle's generation selects the page and `share_pages` freezes completed pages behind
copy-on-write `Arc` wrappers for cheap checkpoint forks. `ConcurrentPagedArena` lets
workers build private `Vec<T>` allocations and publish them into a segmented `OnceLock`
directory without copying or flattening, `GenerationalPagedArena` provides lock-free
slot allocation over `ArcSwapOption` pages, and `OwnedInterner` / `OwnedTextInterner`
provide append-only key and lock-free read-path text interning for durable handles.
*/

use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use arc_swap::ArcSwapOption;

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

impl<T> ArenaPage<T> {
    fn shares_allocation(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Shared { arena: left, .. }, Self::Shared { arena: right, .. }) => {
                Arc::ptr_eq(left, right)
            }
            _ => false,
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

impl<T: Default + Clone + compiler_storage::Record + 'static> compiler_storage::Record
    for ArenaPage<T>
{
    fn write(
        &self,
        output: &mut compiler_storage::RecordWriter,
    ) -> Result<(), compiler_storage::RecordError> {
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
        input: &mut compiler_storage::RecordReader<'_, Context>,
    ) -> Result<Self, compiler_storage::RecordError> {
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
        self.first_generation = source.first_generation;
        self.first_page.clone_from(&source.first_page);
        self.additional_pages.clone_from(&source.additional_pages);
        self.page_positions.clone_from(&source.page_positions);
        self.page_base = source.page_base;
        self.current_generation = source.current_generation;
        self.active_count = source.active_count;
        self.dummy.clone_from(&source.dummy);
    }
}

impl<T: Default> PagedArenaSet<T> {
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

    #[inline]
    pub fn get(&self, handle: Handle<T>) -> &T {
        self.page(handle)
            .map(|page| page.get(handle))
            .unwrap_or(&self.dummy)
    }

    pub fn get_mut(&mut self, handle: Handle<T>) -> &mut T {
        if handle.generation() == self.first_generation {
            return self.first_page.get_mut(handle);
        }
        match self.page_position(handle.generation()) {
            Some(position) => self.additional_pages[position].1.get_mut(handle),
            None => &mut self.dummy,
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

    pub fn span_or_empty(&self, span: HandleSpan<T>) -> &[T] {
        self.span(span).unwrap_or(&[])
    }

    pub fn span_mut(&mut self, span: HandleSpan<T>) -> Option<&mut [T]> {
        if span.is_empty() {
            return Some(&mut []);
        }
        self.page_mut(span.start())?.span_mut(span)
    }

    pub fn span_mut_or_empty(&mut self, span: HandleSpan<T>) -> &mut [T] {
        if span.is_empty() {
            return &mut [];
        }
        self.page_mut(span.start())
            .and_then(|page| page.span_mut(span))
            .unwrap_or(&mut [])
    }

    pub fn shares_first_page(&self, other: &Self) -> bool {
        self.first_generation == other.first_generation
            && self.first_page.shares_allocation(&other.first_page)
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
        self.first_page.reset_retain_capacity();
        self.additional_pages.clear();
        self.page_positions.clear();
        self.page_base = 0;
        self.current_generation = self.first_generation;
        self.active_count = 0;
        self.dummy = T::default();
    }

    pub fn revision(&self) -> u64 {
        self.pages()
            .map(Arena::revision)
            .fold(u64::from(self.current_generation), u64::wrapping_add)
    }

    pub fn append_epoch(&self) -> Option<(u64, usize)> {
        if self.additional_pages.is_empty() && self.current_generation == self.first_generation {
            return self.first_page.append_epoch();
        }
        let current = self.current();
        let current_rewrite = match current {
            Some(page) => page.append_epoch()?.0,
            None => 0,
        };
        let mut epoch = current_rewrite
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            .wrapping_add(u64::from(self.current_generation))
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            .wrapping_add(self.page_count() as u64);
        for page in self.pages() {
            if !current.is_some_and(|cur| std::ptr::eq(page, cur)) {
                page.append_epoch()?;
                epoch = epoch
                    .wrapping_mul(0x9E37_79B9_7F4A_7C15)
                    .wrapping_add(page.revision());
            }
        }
        Some((epoch, self.active_count))
    }

    pub fn for_each_from(&self, start: usize, mut visit: impl FnMut(Handle<T>, &T)) {
        if self.additional_pages.is_empty() && self.current_generation == self.first_generation {
            for (handle, item) in self.first_page.iter_from(start) {
                visit(handle, item);
            }
            return;
        }
        let Some(current) = self.current() else {
            return;
        };
        let prefix_len = self.active_count.saturating_sub(current.len());
        let local_start = start.saturating_sub(prefix_len);
        for (handle, item) in current.iter_from(local_start) {
            visit(handle, item);
        }
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

impl<T> compiler_storage::Record for PagedArenaFrontier<T> {
    fn write(
        &self,
        output: &mut compiler_storage::RecordWriter,
    ) -> Result<(), compiler_storage::RecordError> {
        self.generation.write(output)?;
        self.next_arena_index.write(output)
    }
    fn read<Context>(
        input: &mut compiler_storage::RecordReader<'_, Context>,
    ) -> Result<Self, compiler_storage::RecordError> {
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

impl<T: Default + Clone + compiler_storage::Record + 'static> compiler_storage::Record
    for PagedArenaSet<T>
{
    fn write(
        &self,
        output: &mut compiler_storage::RecordWriter,
    ) -> Result<(), compiler_storage::RecordError> {
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
        input: &mut compiler_storage::RecordReader<'_, Context>,
    ) -> Result<Self, compiler_storage::RecordError> {
        input.record(|input| {
            let mut result = Self {
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

const PAGES_PER_DIRECTORY_SEGMENT: usize = 256;

pub struct ConcurrentPagedArena<T: Default> {
    directory: PageDirectory<T>,
    first_page_identity: u32,
    next_page: AtomicU32,
    published_pages: AtomicU32,
    published_values: AtomicU64,
    default_page_capacity: usize,
    dummy: T,
}

impl<T: Default> ConcurrentPagedArena<T> {
    pub fn new(maximum_pages: usize, default_page_capacity: usize) -> Self {
        Self::new_with_first_page(maximum_pages, default_page_capacity, 1)
    }

    pub fn new_with_first_page(
        maximum_pages: usize,
        default_page_capacity: usize,
        first_page_identity: u32,
    ) -> Self {
        assert!(
            maximum_pages > 0,
            "concurrent paged arena needs at least one page"
        );
        assert!(
            first_page_identity != 0,
            "concurrent paged arena page identity must be nonzero"
        );
        assert!(
            u32::try_from(maximum_pages)
                .ok()
                .and_then(|count| first_page_identity.checked_add(count - 1))
                .is_some(),
            "concurrent paged arena page count exceeds handle identity"
        );

        Self {
            directory: PageDirectory::new(maximum_pages),
            first_page_identity,
            next_page: AtomicU32::new(0),
            published_pages: AtomicU32::new(0),
            published_values: AtomicU64::new(0),
            default_page_capacity,
            dummy: T::default(),
        }
    }

    pub fn reserve_page(&self) -> Option<ConcurrentArenaPage<'_, T>> {
        self.reserve_page_with_capacity(self.default_page_capacity)
    }

    pub fn reserve_page_with_capacity(
        &self,
        capacity: usize,
    ) -> Option<ConcurrentArenaPage<'_, T>> {
        let page_index = self
            .next_page
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |next_page| {
                ((next_page as usize) < self.directory.maximum_pages()).then_some(next_page + 1)
            })
            .ok()?;

        Some(ConcurrentArenaPage {
            arena: self,
            page_index,
            values: Vec::with_capacity(capacity),
        })
    }

    pub fn seal(self) -> Result<SealedPagedArena<T>, IncompletePagePublication> {
        let reserved_pages = self.next_page.load(Ordering::Acquire);
        let published_pages = self.published_pages.load(Ordering::Acquire);
        if reserved_pages != published_pages {
            return Err(IncompletePagePublication {
                reserved_pages,
                published_pages,
            });
        }

        Ok(SealedPagedArena {
            directory: self.directory,
            first_page_identity: self.first_page_identity,
            page_count: published_pages,
            value_count: self.published_values.load(Ordering::Acquire),
            dummy: self.dummy,
        })
    }
}

pub struct ConcurrentArenaPage<'arena, T: Default> {
    arena: &'arena ConcurrentPagedArena<T>,
    page_index: u32,
    values: Vec<T>,
}

impl<T: Default> ConcurrentArenaPage<'_, T> {
    pub const fn page_identity(&self) -> u32 {
        self.arena.first_page_identity + self.page_index
    }

    pub fn push(&mut self, value: T) -> Handle<T> {
        let handle = self.next_handle();
        self.values.push(value);
        handle
    }

    pub fn insert_many(&mut self, values: impl IntoIterator<Item = T>) -> HandleSpan<T> {
        let start = self.next_handle();
        let mut count = 0u32;
        for value in values {
            self.values.push(value);
            count = count
                .checked_add(1)
                .expect("concurrent paged arena span count overflow");
        }
        HandleSpan::from_parts_or_empty(start, count)
    }

    pub fn get(&self, handle: Handle<T>) -> Option<&T> {
        self.local_slot(handle)
            .and_then(|slot| self.values.get(slot))
    }

    pub fn get_mut(&mut self, handle: Handle<T>) -> Option<&mut T> {
        self.local_slot(handle)
            .and_then(|slot| self.values.get_mut(slot))
    }

    pub fn as_slice(&self) -> &[T] {
        &self.values
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn publish(self) -> PublishedArenaPage<T> {
        let Self {
            arena,
            page_index,
            values,
        } = self;
        let value_count = u32::try_from(values.len())
            .expect("concurrent paged arena page length exceeds handle slot identity");
        let page = arena
            .directory
            .page_or_create(page_index as usize)
            .expect("reserved concurrent paged arena page must be in range");
        if page.set(values).is_err() {
            panic!("concurrent paged arena page identity published twice");
        }
        arena
            .published_values
            .fetch_add(u64::from(value_count), Ordering::Release);
        arena.published_pages.fetch_add(1, Ordering::Release);

        PublishedArenaPage {
            page_identity: arena.first_page_identity + page_index,
            value_count,
            marker: PhantomData,
        }
    }

    fn next_handle(&self) -> Handle<T> {
        let slot_identity = self
            .values
            .len()
            .checked_add(1)
            .and_then(|slot| u32::try_from(slot).ok())
            .expect("concurrent paged arena page length exceeds handle slot identity");
        Handle::from_parts(slot_identity, self.page_identity())
    }

    fn local_slot(&self, handle: Handle<T>) -> Option<usize> {
        if handle.generation() != self.page_identity() || !handle.is_valid() {
            return None;
        }
        usize::try_from(handle.arena_index()).ok()?.checked_sub(1)
    }
}

pub struct PublishedArenaPage<T> {
    page_identity: u32,
    value_count: u32,
    marker: PhantomData<fn() -> T>,
}

impl<T> PublishedArenaPage<T> {
    pub const fn len(self) -> usize {
        self.value_count as usize
    }

    pub const fn is_empty(self) -> bool {
        self.value_count == 0
    }
}

impl<T> Clone for PublishedArenaPage<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for PublishedArenaPage<T> {}

pub struct SealedPagedArena<T: Default> {
    directory: PageDirectory<T>,
    first_page_identity: u32,
    page_count: u32,
    value_count: u64,
    dummy: T,
}

impl<T: Default> SealedPagedArena<T> {
    pub fn begin_concurrent_extension(
        self,
        default_page_capacity: usize,
    ) -> ConcurrentPagedArena<T> {
        ConcurrentPagedArena {
            directory: self.directory,
            first_page_identity: self.first_page_identity,
            next_page: AtomicU32::new(self.page_count),
            published_pages: AtomicU32::new(self.page_count),
            published_values: AtomicU64::new(self.value_count),
            default_page_capacity,
            dummy: self.dummy,
        }
    }

    #[inline]
    pub fn get(&self, handle: Handle<T>) -> &T {
        self.resolve(handle).unwrap_or(&self.dummy)
    }

    #[inline]
    pub fn try_get(&self, handle: Handle<T>) -> Option<&T> {
        self.resolve(handle)
    }

    pub fn span(&self, span: HandleSpan<T>) -> Option<&[T]> {
        if span.is_empty() {
            return Some(&[]);
        }

        let page_index = self.page_index(span.start())?;
        let start_slot = self.slot_index(span.start())?;
        let end_slot = start_slot.checked_add(span.len())?;
        self.directory
            .page(page_index)?
            .get()?
            .get(start_slot..end_slot)
    }

    pub fn span_or_empty(&self, span: HandleSpan<T>) -> &[T] {
        self.span(span).unwrap_or(&[])
    }

    pub fn get_mut(&mut self, handle: Handle<T>) -> Option<&mut T> {
        let page_index = self.page_index(handle)?;
        let slot_index = self.slot_index(handle)?;
        self.directory
            .page_mut(page_index)?
            .get_mut()?
            .get_mut(slot_index)
    }

    pub fn span_mut(&mut self, span: HandleSpan<T>) -> Option<&mut [T]> {
        if span.is_empty() {
            return Some(&mut []);
        }

        let page_index = self.page_index(span.start())?;
        let start_slot = self.slot_index(span.start())?;
        let end_slot = start_slot.checked_add(span.len())?;
        self.directory
            .page_mut(page_index)?
            .get_mut()?
            .get_mut(start_slot..end_slot)
    }

    pub fn page(&self, page: PublishedArenaPage<T>) -> Option<&[T]> {
        let page_index =
            usize::try_from(page.page_identity.checked_sub(self.first_page_identity)?).ok()?;
        let values = self.directory.page(page_index)?.get()?;
        (values.len() == page.len()).then_some(values.as_slice())
    }

    pub fn len(&self) -> usize {
        usize::try_from(self.value_count).expect("concurrent paged arena length exceeds usize")
    }

    pub fn is_empty(&self) -> bool {
        self.value_count == 0
    }

    pub const fn page_count(&self) -> usize {
        self.page_count as usize
    }

    pub fn into_pages(self) -> Vec<Vec<T>> {
        self.directory.into_pages(self.page_count as usize)
    }

    fn resolve(&self, handle: Handle<T>) -> Option<&T> {
        let page_index = self.page_index(handle)?;
        let slot_index = self.slot_index(handle)?;
        self.directory.page(page_index)?.get()?.get(slot_index)
    }

    fn page_index(&self, handle: Handle<T>) -> Option<usize> {
        if !handle.is_valid() || handle.generation() == 0 {
            return None;
        }
        let page_index =
            usize::try_from(handle.generation().checked_sub(self.first_page_identity)?).ok()?;
        (page_index < self.page_count as usize).then_some(page_index)
    }

    fn slot_index(&self, handle: Handle<T>) -> Option<usize> {
        usize::try_from(handle.arena_index()).ok()?.checked_sub(1)
    }
}

struct PageDirectory<T> {
    segments: Box<[OnceLock<PageDirectorySegment<T>>]>,
    maximum_pages: usize,
}

impl<T> PageDirectory<T> {
    fn new(maximum_pages: usize) -> Self {
        let segment_count = maximum_pages.div_ceil(PAGES_PER_DIRECTORY_SEGMENT);
        Self {
            segments: (0..segment_count)
                .map(|_| OnceLock::new())
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            maximum_pages,
        }
    }

    const fn maximum_pages(&self) -> usize {
        self.maximum_pages
    }

    fn page_or_create(&self, page_index: usize) -> Option<&OnceLock<Vec<T>>> {
        if page_index >= self.maximum_pages {
            return None;
        }
        let segment_index = page_index / PAGES_PER_DIRECTORY_SEGMENT;
        let page_in_segment = page_index % PAGES_PER_DIRECTORY_SEGMENT;
        self.segments
            .get(segment_index)?
            .get_or_init(PageDirectorySegment::new)
            .pages
            .get(page_in_segment)
    }

    fn page(&self, page_index: usize) -> Option<&OnceLock<Vec<T>>> {
        if page_index >= self.maximum_pages {
            return None;
        }
        let segment_index = page_index / PAGES_PER_DIRECTORY_SEGMENT;
        let page_in_segment = page_index % PAGES_PER_DIRECTORY_SEGMENT;
        self.segments
            .get(segment_index)?
            .get()?
            .pages
            .get(page_in_segment)
    }

    fn page_mut(&mut self, page_index: usize) -> Option<&mut OnceLock<Vec<T>>> {
        if page_index >= self.maximum_pages {
            return None;
        }
        let segment_index = page_index / PAGES_PER_DIRECTORY_SEGMENT;
        let page_in_segment = page_index % PAGES_PER_DIRECTORY_SEGMENT;
        self.segments
            .get_mut(segment_index)?
            .get_mut()?
            .pages
            .get_mut(page_in_segment)
    }

    fn into_pages(mut self, page_count: usize) -> Vec<Vec<T>> {
        let mut pages = Vec::with_capacity(page_count);
        for page_index in 0..page_count {
            let segment_index = page_index / PAGES_PER_DIRECTORY_SEGMENT;
            let page_in_segment = page_index % PAGES_PER_DIRECTORY_SEGMENT;
            let page = self.segments[segment_index]
                .get_mut()
                .expect("sealed concurrent paged arena segment must be published")
                .pages[page_in_segment]
                .take()
                .expect("sealed concurrent paged arena page must be published");
            pages.push(page);
        }
        pages
    }
}

struct PageDirectorySegment<T> {
    pages: Box<[OnceLock<Vec<T>>]>,
}

impl<T> PageDirectorySegment<T> {
    fn new() -> Self {
        Self {
            pages: (0..PAGES_PER_DIRECTORY_SEGMENT)
                .map(|_| OnceLock::new())
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IncompletePagePublication {
    pub reserved_pages: u32,
    pub published_pages: u32,
}

const EMPTY_FREE_STACK: u64 = 0;

struct FreeStack {
    head: AtomicU64,
    next: Box<[AtomicU32]>,
}

impl FreeStack {
    fn new(capacity: usize) -> Self {
        let next = (0..capacity).map(|_| AtomicU32::new(0)).collect();

        Self {
            head: AtomicU64::new(EMPTY_FREE_STACK),
            next,
        }
    }

    fn push(&self, index: u32) {
        debug_assert_ne!(index, 0);
        let slot_index = usize::try_from(index).expect("free stack index overflow");

        loop {
            let head = self.head.load(Ordering::Relaxed);
            let (old_index, old_tag) = unpack_free_stack(head);
            self.next[slot_index].store(old_index, Ordering::Relaxed);
            let new_head = pack_free_stack(index, old_tag.wrapping_add(1));

            if self
                .head
                .compare_exchange_weak(head, new_head, Ordering::Release, Ordering::Relaxed)
                .is_ok()
            {
                return;
            }
        }
    }

    fn pop(&self) -> Option<u32> {
        loop {
            let head = self.head.load(Ordering::Acquire);

            if head == EMPTY_FREE_STACK {
                return None;
            }

            let (index, tag) = unpack_free_stack(head);
            let slot_index = usize::try_from(index).expect("free stack index overflow");
            let next_index = self.next[slot_index].load(Ordering::Relaxed);
            let new_head = if next_index == 0 {
                EMPTY_FREE_STACK
            } else {
                pack_free_stack(next_index, tag.wrapping_add(1))
            };

            if self
                .head
                .compare_exchange_weak(head, new_head, Ordering::AcqRel, Ordering::Relaxed)
                .is_ok()
            {
                return Some(index);
            }
        }
    }
}

#[inline]
fn pack_free_stack(index: u32, tag: u32) -> u64 {
    u64::from(index) | (u64::from(tag) << 32)
}

#[inline]
fn unpack_free_stack(value: u64) -> (u32, u32) {
    (value as u32, (value >> 32) as u32)
}

const SLOT_EMPTY: u8 = 0;
const SLOT_ACTIVE: u8 = 1;
const SLOT_FREED: u8 = 2;

const PAGE_SIZE: usize = 16;

pub struct GenerationalPagedArena<T: Default> {
    pages: Box<[ArcSwapOption<Page<T>>]>,
    slot_metadata: Box<[AtomicU64]>,
    dummy_page: Arc<Page<T>>,
    free_stack: FreeStack,
}

impl<T: Default> GenerationalPagedArena<T> {
    pub fn new(max_capacity: usize) -> Self {
        assert!(
            max_capacity >= PAGE_SIZE,
            "generational paged arena max capacity must be at least {PAGE_SIZE}"
        );

        let page_count = max_capacity.div_ceil(PAGE_SIZE);
        let dummy_page = Arc::new(Page::new());

        let pages = (0..page_count)
            .map(|page_index| {
                if page_index == 0 {
                    ArcSwapOption::from(Some(Arc::clone(&dummy_page)))
                } else {
                    ArcSwapOption::from(None)
                }
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();

        let total_slots = page_count * PAGE_SIZE;
        let slot_metadata = (0..total_slots)
            .map(|slot_index| {
                let state = if slot_index == 0 {
                    SLOT_ACTIVE
                } else {
                    SLOT_EMPTY
                };

                AtomicU64::new(pack_slot(state, 0))
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();

        Self {
            pages,
            slot_metadata,
            dummy_page,
            free_stack: FreeStack::new(total_slots),
        }
    }

    pub fn alloc_default(&self) -> Handle<T> {
        self.alloc()
    }

    pub fn alloc(&self) -> Handle<T> {
        while let Some(index) = self.free_stack.pop() {
            let Some((page_index, _slot_index)) = self.decompose(index) else {
                continue;
            };

            if self.load_or_alloc_page(page_index).is_none() {
                continue;
            }

            if let Some(handle) = self.claim_slot(index, SLOT_FREED) {
                return handle;
            }
        }

        for page_index in 0..self.pages.len() {
            if self.load_or_alloc_page(page_index).is_none() {
                continue;
            }

            let start_slot = if page_index == 0 { 1 } else { 0 };
            for slot_index in start_slot..PAGE_SIZE {
                let index = self.compose(page_index, slot_index);
                if let Some(handle) = self.claim_slot(index, SLOT_EMPTY) {
                    return handle;
                }
            }
        }

        Handle::invalid()
    }

    pub fn free(&self, handle: Handle<T>) -> bool {
        if !handle.is_valid() {
            return false;
        }

        let Some((_, _)) = self.decompose(handle.arena_index()) else {
            return false;
        };

        let slot_metadata = &self.slot_metadata[handle.arena_index() as usize];

        loop {
            let current_metadata = slot_metadata.load(Ordering::Acquire);
            let (state, generation) = unpack_slot(current_metadata);

            if state != SLOT_ACTIVE || generation != handle.generation() {
                return false;
            }

            let freed_metadata = pack_slot(SLOT_FREED, generation);
            if slot_metadata
                .compare_exchange(
                    current_metadata,
                    freed_metadata,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                self.free_stack.push(handle.arena_index());
                return true;
            }
        }
    }

    pub fn get(&self, handle: Handle<T>) -> SlotRef<T> {
        if !handle.is_valid() {
            return self.dummy();
        }

        let Some((page_index, slot_index)) = self.decompose(handle.arena_index()) else {
            return self.dummy();
        };

        let (state, generation) =
            unpack_slot(self.slot_metadata[handle.arena_index() as usize].load(Ordering::Acquire));

        if state != SLOT_ACTIVE || generation != handle.generation() {
            return self.dummy();
        }

        let Some(page) = self.load_page(page_index) else {
            return self.dummy();
        };

        SlotRef {
            page,
            slot_index,
            dummy: false,
            marker: PhantomData,
        }
    }

    pub fn is_valid(&self, handle: Handle<T>) -> bool {
        !self.get(handle).is_dummy()
    }

    pub fn dummy(&self) -> SlotRef<T> {
        SlotRef {
            page: Arc::clone(&self.dummy_page),
            slot_index: 0,
            dummy: true,
            marker: PhantomData,
        }
    }

    pub fn active_count(&self) -> usize {
        self.slot_metadata
            .iter()
            .enumerate()
            .skip(1)
            .filter(|(_, slot_metadata)| {
                let (state, _) = unpack_slot(slot_metadata.load(Ordering::Relaxed));
                state == SLOT_ACTIVE
            })
            .count()
    }

    pub fn capacity(&self) -> usize {
        self.pages
            .iter()
            .filter(|page_slot| page_slot.load().is_some())
            .count()
            * PAGE_SIZE
    }

    pub fn max_capacity(&self) -> usize {
        self.pages.len() * PAGE_SIZE
    }

    pub fn page_size(&self) -> usize {
        PAGE_SIZE
    }

    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    pub fn active_handles(&self) -> Vec<Handle<T>> {
        self.slot_metadata
            .iter()
            .enumerate()
            .skip(1)
            .filter_map(|(index, slot_metadata)| {
                let (state, generation) = unpack_slot(slot_metadata.load(Ordering::Acquire));

                if state == SLOT_ACTIVE {
                    Some(Handle::from_parts(
                        u32::try_from(index).expect("generational paged arena index overflow"),
                        generation,
                    ))
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn reclaim_empty_pages(&self) -> usize {
        0
    }

    fn claim_slot(&self, index: u32, expected_state: u8) -> Option<Handle<T>> {
        let slot_metadata = &self.slot_metadata[index as usize];

        loop {
            let current_metadata = slot_metadata.load(Ordering::Acquire);
            let (state, generation) = unpack_slot(current_metadata);

            if state != expected_state {
                return None;
            }

            let next_generation = generation
                .checked_add(1)
                .expect("generational paged arena generation overflow");
            let active_metadata = pack_slot(SLOT_ACTIVE, next_generation);

            if slot_metadata
                .compare_exchange(
                    current_metadata,
                    active_metadata,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                return Some(Handle::from_parts(index, next_generation));
            }
        }
    }

    fn load_or_alloc_page(&self, page_index: usize) -> Option<Arc<Page<T>>> {
        let page_slot = self.pages.get(page_index)?;

        if let Some(page) = page_slot.load_full() {
            return Some(page);
        }

        let new_page = Arc::new(Page::new());
        page_slot.rcu(|current| {
            current
                .as_ref()
                .map(Arc::clone)
                .or_else(|| Some(Arc::clone(&new_page)))
        });

        page_slot.load_full()
    }

    fn load_page(&self, page_index: usize) -> Option<Arc<Page<T>>> {
        self.pages.get(page_index)?.load_full()
    }

    fn compose(&self, page_index: usize, slot_index: usize) -> u32 {
        u32::try_from(page_index * PAGE_SIZE + slot_index)
            .expect("generational paged arena index overflow")
    }

    fn decompose(&self, index: u32) -> Option<(usize, usize)> {
        if index == 0 {
            return None;
        }

        let flat_index = usize::try_from(index).ok()?;
        let page_index = flat_index / PAGE_SIZE;
        let slot_index = flat_index % PAGE_SIZE;

        if page_index >= self.pages.len() {
            return None;
        }

        Some((page_index, slot_index))
    }
}

struct Page<T: Default> {
    entries: Vec<T>,
}

impl<T: Default> Page<T> {
    fn new() -> Self {
        Self {
            entries: (0..PAGE_SIZE).map(|_| T::default()).collect(),
        }
    }
}

pub struct SlotRef<T: Default> {
    page: Arc<Page<T>>,
    slot_index: usize,
    dummy: bool,
    marker: PhantomData<fn() -> T>,
}

impl<T: Default> SlotRef<T> {
    pub fn is_dummy(&self) -> bool {
        self.dummy
    }

    pub fn slot_index(&self) -> usize {
        self.slot_index
    }

    pub fn value(&self) -> &T {
        &self.page.entries[self.slot_index]
    }
}

#[inline]
fn pack_slot(state: u8, generation: u32) -> u64 {
    u64::from(state) | (u64::from(generation) << 32)
}

#[inline]
fn unpack_slot(metadata: u64) -> (u8, u32) {
    (metadata as u8, (metadata >> 32) as u32)
}

#[derive(compiler_storage::Record)]
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

impl<T> PartialEq for OwnedInterner<T> {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl<T> Eq for OwnedInterner<T> {}

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
        let interner = Self {
            texts: [const { OnceLock::new() }; TEXT_PAGES],
            indexes: [const { OnceLock::new() }; INDEX_GENERATIONS],
            generation: AtomicUsize::new(0),
            minted: Mutex::new(0),
        };
        let _ = interner.indexes[0].set(TextIndex::with_words(FIRST_INDEX_WORDS));
        interner
    }
}

impl OwnedTextInterner {
    pub fn intern(&self, text: &str) -> usize {
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
        assert!(
            slot < u32::MAX as usize,
            "text interner slot space exhausted"
        );
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

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn texts_in_order(&self) -> Vec<&str> {
        let minted = self
            .minted
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        (0..*minted)
            .map(|slot| self.text(slot).expect("a minted slot publishes its text"))
            .collect()
    }

    fn from_texts<'text>(texts: impl IntoIterator<Item = &'text str>) -> Self {
        let interner = Self::default();
        for text in texts {
            interner.intern(text);
        }
        interner
    }

    fn current_index(&self) -> &TextIndex {
        self.index(self.generation.load(Ordering::Acquire))
    }

    fn index(&self, generation: usize) -> &TextIndex {
        self.indexes[generation]
            .get()
            .expect("the current index generation is published before it is named")
    }
}

impl TextIndex {
    fn with_words(count: usize) -> Self {
        Self {
            words: (0..count).map(|_| AtomicU64::new(0)).collect(),
        }
    }

    fn probe(&self, tag: u32, text: &str, interner: &OwnedTextInterner) -> Probe {
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
        Self::from_texts(self.texts_in_order())
    }
}

impl PartialEq for OwnedTextInterner {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Eq for OwnedTextInterner {}

impl std::fmt::Debug for OwnedTextInterner {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("OwnedTextInterner")
    }
}

fn write_texts(
    output: &mut compiler_storage::RecordWriter,
    texts: &[&str],
) -> Result<(), compiler_storage::RecordError> {
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
        output: &mut compiler_storage::RecordWriter,
        numbers: &[u32],
    ) -> Result<(), compiler_storage::RecordError> {
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

impl compiler_storage::Record for OwnedTextInterner {
    fn write(
        &self,
        output: &mut compiler_storage::RecordWriter,
    ) -> Result<(), compiler_storage::RecordError> {
        write_texts(output, &self.texts_in_order())
    }

    fn read<Context>(
        input: &mut compiler_storage::RecordReader<'_, Context>,
    ) -> Result<Self, compiler_storage::RecordError> {
        let texts = Vec::<Box<str>>::read(input)?;
        Ok(Self::from_texts(texts.iter().map(AsRef::as_ref)))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Barrier};

    use super::{
        ConcurrentPagedArena, FIRST_INDEX_WORDS, FIRST_TEXT_PAGE, GenerationalPagedArena,
        OwnedInterner, OwnedTextInterner, PagedArenaSet,
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
        let limits = compiler_storage::RecordLimits::default();
        let bytes = compiler_storage::encode(&original, limits).unwrap();
        let mut restored: PagedArenaSet<String> = compiler_storage::decode(&bytes, limits).unwrap();

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
        let limits = compiler_storage::RecordLimits::default();
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
            let bytes = compiler_storage::encode(&value, limits).unwrap();
            assert!(compiler_storage::decode::<PagedArenaSet<u32>>(&bytes, limits).is_err());
        }
    }

    #[test]
    fn publication_preserves_the_worker_vector_allocation() {
        let arena = ConcurrentPagedArena::<String>::new(4, 8);
        let mut page = arena.reserve_page().expect("page identity");
        let first = page.push("alpha".to_owned());
        let values = page.insert_many(["beta".to_owned(), "gamma".to_owned()]);
        let allocation = page.as_slice().as_ptr();
        let published = page.publish();

        let sealed = arena.seal().expect("all pages published");
        assert_eq!(
            sealed.page(published).expect("published page").as_ptr(),
            allocation
        );
        assert_eq!(sealed.get(first), "alpha");
        assert_eq!(sealed.span_or_empty(values), ["beta", "gamma"]);
    }

    #[test]
    fn workers_publish_disjoint_pages_under_contention() {
        const WORKERS: usize = 8;
        const VALUES_PER_WORKER: usize = 64;

        let arena = ConcurrentPagedArena::<u64>::new(WORKERS, VALUES_PER_WORKER);
        let barrier = Arc::new(Barrier::new(WORKERS));
        let spans = std::thread::scope(|scope| {
            let workers = (0..WORKERS)
                .map(|worker| {
                    let arena = &arena;
                    let barrier = Arc::clone(&barrier);
                    scope.spawn(move || {
                        barrier.wait();
                        let mut page = arena.reserve_page().expect("worker page");
                        let span = page.insert_many(
                            (0..VALUES_PER_WORKER)
                                .map(|position| (worker * VALUES_PER_WORKER + position) as u64),
                        );
                        page.publish();
                        (worker, span)
                    })
                })
                .collect::<Vec<_>>();
            workers
                .into_iter()
                .map(|worker| worker.join().expect("worker completed"))
                .collect::<Vec<_>>()
        });

        let sealed = arena.seal().expect("all worker pages published");
        assert_eq!(sealed.page_count(), WORKERS);
        assert_eq!(sealed.len(), WORKERS * VALUES_PER_WORKER);
        for (worker, span) in spans {
            let expected = (0..VALUES_PER_WORKER)
                .map(|position| (worker * VALUES_PER_WORKER + position) as u64)
                .collect::<Vec<_>>();
            assert_eq!(sealed.span_or_empty(span), expected);
        }
    }

    #[test]
    fn large_page_bound_allocates_only_the_top_level_directory() {
        let arena = ConcurrentPagedArena::<u64>::new(1_000_000, 64);

        assert_eq!(arena.directory.segments.len(), 1_000_000usize.div_ceil(256));
        assert!(
            arena
                .directory
                .segments
                .iter()
                .all(|segment| segment.get().is_none())
        );

        let page = arena.reserve_page().expect("page identity");
        page.publish();
        assert!(arena.directory.segments[0].get().is_some());
        assert!(
            arena.directory.segments[1..]
                .iter()
                .all(|segment| segment.get().is_none())
        );
    }

    #[test]
    fn private_pages_allow_rewrites_before_publication() {
        let arena = ConcurrentPagedArena::<String>::new(1, 2);
        let mut page = arena.reserve_page().expect("page identity");
        let value = page.push("before".to_owned());
        *page.get_mut(value).expect("private value") = "after".to_owned();
        assert_eq!(page.get(value).map(String::as_str), Some("after"));
        page.publish();

        let sealed = arena.seal().expect("page published");
        assert_eq!(sealed.get(value), "after");
    }

    #[test]
    fn sealed_pages_allow_exclusive_later_stage_rewrites() {
        let arena = ConcurrentPagedArena::<String>::new(1, 3);
        let mut page = arena.reserve_page().expect("page identity");
        let first = page.push("alpha".to_owned());
        let values = page.insert_many(["beta".to_owned(), "gamma".to_owned()]);
        page.publish();

        let mut sealed = arena.seal().expect("page published");
        *sealed.get_mut(first).expect("published value") = "delta".to_owned();
        sealed.span_mut(values).expect("published span").reverse();

        assert_eq!(sealed.get(first), "delta");
        assert_eq!(sealed.span_or_empty(values), ["gamma", "beta"]);
    }

    #[test]
    fn later_concurrent_epoch_preserves_existing_pages_and_handles() {
        let arena = ConcurrentPagedArena::<String>::new(4, 2);
        let mut first_page = arena.reserve_page().expect("first page identity");
        let first = first_page.push("alpha".to_owned());
        let first_allocation = first_page.as_slice().as_ptr();
        let first_page = first_page.publish();

        let arena = arena
            .seal()
            .expect("first epoch published")
            .begin_concurrent_extension(2);
        let mut second_page = arena.reserve_page().expect("second page identity");
        let second = second_page.push("beta".to_owned());
        second_page.publish();

        let sealed = arena.seal().expect("second epoch published");
        assert_eq!(sealed.get(first), "alpha");
        assert_eq!(sealed.get(second), "beta");
        assert_eq!(
            sealed
                .page(first_page)
                .expect("first published page")
                .as_ptr(),
            first_allocation
        );
        assert_ne!(first.generation(), second.generation());
    }

    #[test]
    fn sealed_page_extraction_preserves_page_order_and_allocations() {
        let arena = ConcurrentPagedArena::<String>::new(2, 2);
        let mut first = arena.reserve_page().expect("first page");
        first.push("alpha".to_owned());
        let first_allocation = first.as_slice().as_ptr();
        let mut second = arena.reserve_page().expect("second page");
        second.push("beta".to_owned());
        first.publish();
        second.publish();

        let pages = arena.seal().expect("pages published").into_pages();
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0], ["alpha"]);
        assert_eq!(pages[1], ["beta"]);
        assert_eq!(pages[0].as_ptr(), first_allocation);
    }

    #[test]
    fn dropped_reservation_refuses_sealing() {
        let arena = ConcurrentPagedArena::<u32>::new(1, 4);
        drop(arena.reserve_page().expect("page identity"));

        let error = arena.seal().err().expect("publication must be incomplete");
        assert_eq!(error.reserved_pages, 1);
        assert_eq!(error.published_pages, 0);
    }

    #[test]
    fn invalid_and_foreign_page_handles_resolve_to_dummy() {
        let arena = ConcurrentPagedArena::<u32>::new(1, 1);
        let mut page = arena.reserve_page().expect("page identity");
        let value = page.push(42);
        page.publish();
        let sealed = arena.seal().expect("page published");

        assert_eq!(*sealed.get(Handle::invalid()), 0);
        assert_eq!(*sealed.get(Handle::from_parts(value.arena_index(), 2)), 0);
    }

    #[test]
    fn resolves_invalid_handles_to_dummy() {
        let arena = GenerationalPagedArena::<String>::new(16);
        let invalid = Handle::<String>::invalid();
        let slot = arena.get(invalid);

        assert!(slot.is_dummy());
        assert_eq!(slot.value(), "");
        assert!(!arena.is_valid(invalid));
    }

    #[test]
    fn allocates_default_slots_and_tracks_active_handles() {
        let arena = GenerationalPagedArena::<String>::new(16);
        let first = arena.alloc_default();
        let second = arena.alloc_default();

        assert_eq!(first.arena_index(), 1);
        assert_eq!(second.arena_index(), 2);
        assert_eq!(arena.active_count(), 2);
        assert_eq!(arena.active_handles(), vec![first, second]);
        assert_eq!(arena.get(first).value(), "");
        assert_eq!(arena.get(second).value(), "");
    }

    #[test]
    fn frees_and_reuses_slots_with_new_generation() {
        let arena = GenerationalPagedArena::<String>::new(16);
        let first = arena.alloc_default();

        assert!(arena.is_valid(first));
        assert!(arena.free(first));
        assert!(!arena.is_valid(first));
        assert!(arena.get(first).is_dummy());

        let reused = arena.alloc_default();

        assert_eq!(reused.arena_index(), first.arena_index());
        assert_ne!(reused.generation(), first.generation());
        assert!(!arena.is_valid(first));
        assert!(arena.is_valid(reused));
    }

    #[test]
    fn grows_pages_on_demand_without_allocating_all_pages() {
        let arena = GenerationalPagedArena::<String>::new(64);

        assert_eq!(arena.capacity(), 16);
        assert_eq!(arena.max_capacity(), 64);

        let handles = (0..20).map(|_| arena.alloc_default()).collect::<Vec<_>>();

        assert!(handles.iter().all(|handle| handle.is_valid()));
        assert_eq!(arena.active_count(), 20);
        assert_eq!(arena.capacity(), 32);
    }

    #[test]
    fn returns_invalid_handle_when_full() {
        let arena = GenerationalPagedArena::<String>::new(16);
        let handles = (0..15).map(|_| arena.alloc_default()).collect::<Vec<_>>();
        let overflow = arena.alloc_default();

        assert!(handles.iter().all(|handle| handle.is_valid()));
        assert!(!overflow.is_valid());
        assert_eq!(arena.active_count(), 15);
    }

    #[test]
    fn page_reclamation_is_disabled_until_quiescence_exists() {
        let arena = GenerationalPagedArena::<String>::new(32);
        let handles = (0..20).map(|_| arena.alloc_default()).collect::<Vec<_>>();

        for handle in handles {
            assert!(arena.free(handle));
        }

        assert_eq!(arena.reclaim_empty_pages(), 0);
        assert_eq!(arena.capacity(), 32);
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
        let decoded: OwnedTextInterner = compiler_storage::decode(
            &compiler_storage::encode(&interner, Default::default()).unwrap(),
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
}
