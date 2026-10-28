//! Several dense arena pages behind one typed handle surface.
//!
//! Each page keeps its worker-produced `Vec` allocations. A handle's generation
//! selects the page and its arena index selects the page-local slot. Semantic
//! spans remain within one page and therefore continue to resolve as slices.

mod page;
use page::ArenaPage;
use std::marker::PhantomData;

use crate::{Arena, ArenaSpanInserter, Handle, HandleSpan};

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

// An empty first page: generation 1, next one-based slot 1.
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
    current_generation: u32,
    active_count: usize,
    dummy: T,
}

// Replacing a set keeps each page's allocation where the page survives, so a
// pass that restores a baseline every sweep does not reallocate it.
impl<T: Default + Clone> Clone for PagedArenaSet<T> {
    fn clone(&self) -> Self {
        Self {
            first_generation: self.first_generation,
            first_page: self.first_page.clone(),
            additional_pages: self.additional_pages.clone(),
            current_generation: self.current_generation,
            active_count: self.active_count,
            dummy: self.dummy.clone(),
        }
    }

    fn clone_from(&mut self, source: &Self) {
        self.first_generation = source.first_generation;
        self.first_page.clone_from(&source.first_page);
        self.additional_pages.clone_from(&source.additional_pages);
        self.current_generation = source.current_generation;
        self.active_count = source.active_count;
        self.dummy.clone_from(&source.dummy);
    }
}

impl<T: Default> PagedArenaSet<T> {
    /// Freeze nonempty payload pages for cheap independent checkpoint forks.
    /// Appends to newly adopted pages stay owned; a later edit detaches only
    /// the edited page. Empty pages remain inline and allocate nothing.
    pub fn share_pages(&mut self)
    where
        T: Clone,
    {
        self.first_page.share();
        for (_, page) in &mut self.additional_pages {
            page.share();
        }
    }

    /// Contextual row equality includes unused page slots and all arena metadata.
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
}

impl<T: Default> PagedArenaSet<T> {
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
            current_generation: generation,
            active_count: 0,
            dummy: T::default(),
        }
    }

    /// Move independently produced pages into this set without copying rows.
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

    /// Move every page after the first into a new set whose first page is
    /// empty, leaving this set with only its first page. A worker that built
    /// pages over a copied baseline hands back just the pages it produced.
    pub fn take_additional_pages(&mut self) -> Self {
        let mut taken = Self::with_generation(self.first_generation);
        taken.additional_pages = std::mem::take(&mut self.additional_pages);
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

    /// Adopt every page of `other` after its first, without copying rows or
    /// remapping handles. Generations order pages, so adoption order is free.
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

    /// Start a later append page after previously published pages.
    /// Pages are zero-allocated on `begin_page`; the page entry is materialized
    /// lazily on first write (`current_mut`).
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

    /// Generation retained when later pages are handed to another owner.
    pub fn first_generation(&self) -> u32 {
        self.first_generation
    }

    pub fn reserve(&mut self, additional: usize) {
        self.current_mut().reserve(additional);
    }

    /// A handle's generation names its page, so a page never hands a freed
    /// slot out again under a bumped generation: insertion always appends.
    pub fn insert(&mut self, item: T) -> Handle<T> {
        self.append(item)
    }

    /// Retire one row. Its handle stops resolving and the slot is never
    /// reused, so later rows keep their page identity.
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
        // A stale tail span is relocated by copying its rows, so count the
        // page's growth rather than one appended row.
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
        let generation = handle.generation();
        if generation == self.first_generation {
            return self.first_page.get_mut(handle);
        }
        if generation < self.first_generation {
            return &mut self.dummy;
        }
        let dense_index = usize::try_from(generation - self.first_generation - 1).ok();
        let index = if self.additional_pages.last().map(|(page_gen, _)| *page_gen) == Some(generation) {
            Some(self.additional_pages.len() - 1)
        } else if let Some(idx) = dense_index
            && self.additional_pages.get(idx).map(|(page_gen, _)| *page_gen) == Some(generation)
        {
            Some(idx)
        } else {
            self.additional_pages
                .binary_search_by_key(&generation, |(page_gen, _)| *page_gen)
                .ok()
        };
        if let Some(idx) = index {
            return self.additional_pages[idx].1.get_mut(handle);
        }
        &mut self.dummy
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

    /// Restore this page's append frontier. Unpublished suffix handles must
    /// not escape; existing rows and the page roster must remain untouched.
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

    /// Rows appended since `frontier`, as one span. Appends always land on
    /// the current page, so a frontier taken on an earlier page means every
    /// row since then is the current page's.
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

    /// Empty every page while keeping the first page's allocation, as a
    /// fresh set would start, including a dummy no lookup has rewritten.
    pub fn reset_retain_capacity(&mut self) {
        self.first_page.reset_retain_capacity();
        self.additional_pages.clear();
        self.current_generation = self.first_generation;
        self.active_count = 0;
        self.dummy = T::default();
    }

    /// Freshness key over every page. Each mutation moves one page's own
    /// revision forward and adoption adds pages, so the sum only grows.
    pub fn revision(&self) -> u64 {
        self.pages()
            .map(Arena::revision)
            .fold(u64::from(self.current_generation), u64::wrapping_add)
    }

    /// Append-only epoch `(rewrite_epoch, active_count)` when every page has
    /// dense append-only storage (`free_indices` empty).
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
                if page.append_epoch().is_none() {
                    return None;
                }
                epoch = epoch
                    .wrapping_mul(0x9E37_79B9_7F4A_7C15)
                    .wrapping_add(page.revision());
            }
        }
        Some((epoch, self.active_count))
    }

    /// Visit rows appended to the current page starting at the 0-based set
    /// offset `start`. Valid when `append_epoch` shares its `rewrite_epoch`
    /// with the prior observation at `start`.
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

    /// Handle of a row borrowed from this set, by locating its page.
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
        if self.additional_pages.last().map(|(page_gen, _)| *page_gen) == Some(generation) {
            let last_idx = self.additional_pages.len() - 1;
            return &mut self.additional_pages[last_idx].1;
        }
        if self
            .additional_pages
            .last()
            .is_none_or(|(last_gen, _)| *last_gen < generation)
        {
            self.additional_pages
                .push((generation, Arena::with_generation(generation).into()));
            let last_idx = self.additional_pages.len() - 1;
            return &mut self.additional_pages[last_idx].1;
        }
        let idx = match self
            .additional_pages
            .binary_search_by_key(&generation, |(page_gen, _)| *page_gen)
        {
            Ok(idx) => idx,
            Err(pos) => {
                self.additional_pages
                    .insert(pos, (generation, Arena::with_generation(generation).into()));
                pos
            }
        };
        &mut self.additional_pages[idx].1
    }

    fn page(&self, handle: Handle<T>) -> Option<&Arena<T>> {
        self.page_for_generation(handle.generation())
    }

    fn page_mut(&mut self, handle: Handle<T>) -> Option<&mut Arena<T>> {
        self.page_for_generation_mut(handle.generation())
    }

    fn page_for_generation(&self, generation: u32) -> Option<&Arena<T>> {
        if generation == self.first_generation {
            return Some(&self.first_page);
        }
        if generation < self.first_generation {
            return None;
        }
        if let Some((last_gen, page)) = self.additional_pages.last()
            && *last_gen == generation
        {
            return Some(page);
        }
        let dense_index = usize::try_from(generation - self.first_generation - 1).ok()?;
        if let Some((page_gen, page)) = self.additional_pages.get(dense_index)
            && *page_gen == generation
        {
            return Some(page);
        }
        let idx = self
            .additional_pages
            .binary_search_by_key(&generation, |(page_gen, _)| *page_gen)
            .ok()?;
        Some(&self.additional_pages[idx].1)
    }

    fn page_for_generation_mut(&mut self, generation: u32) -> Option<&mut Arena<T>> {
        if generation == self.first_generation {
            return Some(&mut self.first_page);
        }
        if generation < self.first_generation {
            return None;
        }
        let dense_index = usize::try_from(generation - self.first_generation - 1).ok()?;
        let index = if self.additional_pages.last().map(|(page_gen, _)| *page_gen) == Some(generation) {
            self.additional_pages.len() - 1
        } else if self
            .additional_pages
            .get(dense_index)
            .map(|(page_gen, _)| *page_gen)
            == Some(generation)
        {
            dense_index
        } else {
            self.additional_pages
                .binary_search_by_key(&generation, |(page_gen, _)| *page_gen)
                .ok()?
        };
        Some(&mut self.additional_pages[index].1)
    }

    fn insert_page(&mut self, generation: u32, page: ArenaPage<T>) {
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
            return;
        }
        match self
            .additional_pages
            .binary_search_by_key(&generation, |(page_gen, _)| *page_gen)
        {
            Ok(_) => panic!("paged arena generation {generation} already exists"),
            Err(pos) => self.additional_pages.insert(pos, (generation, page)),
        }
    }
}

impl<T: Default> Default for PagedArenaSet<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::PagedArenaSet;

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
    }
}
