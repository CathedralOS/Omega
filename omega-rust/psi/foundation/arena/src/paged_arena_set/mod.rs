//! Several dense arena pages behind one typed handle surface.
//!
//! Each page keeps its worker-produced `Vec` allocations. A handle's generation
//! selects the page and its arena index selects the page-local slot. Semantic
//! spans remain within one page and therefore continue to resolve as slices.

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PagedArenaSet<T: Default> {
    first_generation: u32,
    first_page: Arena<T>,
    additional_pages: Vec<Option<Box<Arena<T>>>>,
    current_generation: u32,
    active_count: usize,
    dummy: T,
}

impl<T: Default> PagedArenaSet<T> {
    pub fn new() -> Self {
        Self::with_generation(1)
    }

    pub fn with_generation(generation: u32) -> Self {
        assert!(
            generation != 0,
            "paged arena set generation must be nonzero"
        );
        Self {
            first_generation: generation,
            first_page: Arena::with_generation(generation),
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
        for (index, page) in other.additional_pages.into_iter().enumerate() {
            let Some(page) = page else { continue };
            let generation = other
                .first_generation
                .checked_add(
                    u32::try_from(index + 1).expect("paged arena generation offset overflow"),
                )
                .expect("paged arena generation overflow");
            self.insert_page(generation, *page);
        }
        self.current_generation = self.current_generation.max(other_current_generation);
        self.active_count = self
            .active_count
            .checked_add(other_active_count)
            .expect("paged arena active count overflow");
    }

    /// Start a later append page after previously published pages.
    pub fn begin_page(&mut self, generation: u32) {
        assert!(
            generation != 0,
            "paged arena set generation must be nonzero"
        );
        assert!(
            generation >= self.first_generation,
            "paged arena extension cannot precede retained pages"
        );
        self.insert_page(generation, Arena::with_generation(generation));
        self.current_generation = generation;
    }

    pub fn reserve(&mut self, additional: usize) {
        self.current_mut().reserve(additional);
    }

    pub fn insert(&mut self, item: T) -> Handle<T> {
        let handle = self.current_mut().insert(item);
        self.active_count += 1;
        handle
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
        let handle = self.current_mut().append_to_span(span, item);
        self.active_count += 1;
        handle
    }

    pub fn insert_many(&mut self, items: impl IntoIterator<Item = T>) -> HandleSpan<T> {
        let before = self.current().len();
        let span = self.current_mut().insert_many(items);
        self.active_count += self.current().len() - before;
        span
    }

    pub fn try_insert_many_with<E>(
        &mut self,
        insert_items: impl FnOnce(&mut ArenaSpanInserter<'_, T>) -> Result<(), E>,
    ) -> Result<HandleSpan<T>, E> {
        let before = self.current().len();
        let span = self.current_mut().try_insert_many_with(insert_items)?;
        self.active_count += self.current().len() - before;
        Ok(span)
    }

    pub fn try_insert_many<E>(
        &mut self,
        items: impl IntoIterator<Item = Result<T, E>>,
    ) -> Result<HandleSpan<T>, E> {
        let before = self.current().len();
        let span = self.current_mut().try_insert_many(items)?;
        self.active_count += self.current().len() - before;
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
        if let Some(index) = handle
            .generation()
            .checked_sub(self.first_generation)
            .and_then(|offset| usize::try_from(offset).ok())
            .and_then(|offset| offset.checked_sub(1))
            && let Some(page) = self
                .additional_pages
                .get_mut(index)
                .and_then(|page| page.as_deref_mut())
        {
            return page.get_mut(handle);
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
            .chain(
                self.additional_pages
                    .iter()
                    .filter_map(|page| page.as_deref()),
            )
            .flat_map(Arena::iter)
    }

    pub fn for_each_mut(&mut self, mut visit: impl FnMut(Handle<T>, &mut T)) {
        self.first_page.for_each_mut(&mut visit);
        for page in self
            .additional_pages
            .iter_mut()
            .filter_map(|page| page.as_deref_mut())
        {
            page.for_each_mut(&mut visit);
        }
    }

    pub fn page_count(&self) -> usize {
        1 + self
            .additional_pages
            .iter()
            .filter(|page| page.is_some())
            .count()
    }

    pub fn frontier(&self) -> PagedArenaFrontier<T> {
        let next_arena_index = self
            .current()
            .storage_slice()
            .len()
            .checked_add(1)
            .and_then(|index| u32::try_from(index).ok())
            .expect("paged arena frontier index overflow");
        PagedArenaFrontier {
            generation: self.current_generation,
            next_arena_index,
            marker: PhantomData,
        }
    }

    fn current(&self) -> &Arena<T> {
        self.page_for_generation(self.current_generation)
            .expect("paged arena current generation must exist")
    }

    fn current_mut(&mut self) -> &mut Arena<T> {
        self.page_for_generation_mut(self.current_generation)
            .expect("paged arena current generation must exist")
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
        let index = usize::try_from(generation.checked_sub(self.first_generation)?).ok()?;
        self.additional_pages
            .get(index.checked_sub(1)?)?
            .as_deref()
    }

    fn page_for_generation_mut(&mut self, generation: u32) -> Option<&mut Arena<T>> {
        if generation == self.first_generation {
            return Some(&mut self.first_page);
        }
        let index = usize::try_from(generation.checked_sub(self.first_generation)?).ok()?;
        self.additional_pages
            .get_mut(index.checked_sub(1)?)?
            .as_deref_mut()
    }

    fn insert_page(&mut self, generation: u32, page: Arena<T>) {
        assert!(
            generation > self.first_generation,
            "paged arena generation adopted twice or precedes retained pages"
        );
        let index = usize::try_from(generation - self.first_generation - 1)
            .expect("paged arena generation offset exceeds usize");
        if self.additional_pages.len() <= index {
            self.additional_pages.resize_with(index + 1, || None);
        }
        assert!(
            self.additional_pages[index].is_none(),
            "paged arena generation adopted twice"
        );
        self.additional_pages[index] = Some(Box::new(page));
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
}
