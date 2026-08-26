use std::marker::PhantomData;
use std::ops::Range;

use crate::{Handle, HandleSpan};

const DEFAULT_PAGE_SIZE: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PagedArena<T> {
    pages: Vec<Vec<T>>,
    page_size: usize,
    len: usize,
    dummy: T,
}

impl<T: Default> PagedArena<T> {
    pub fn new() -> Self {
        Self::with_page_size(DEFAULT_PAGE_SIZE)
    }

    pub fn with_page_size(page_size: usize) -> Self {
        assert!(
            page_size > 0,
            "paged arena page size must be greater than zero"
        );

        Self {
            pages: Vec::new(),
            page_size,
            len: 0,
            dummy: T::default(),
        }
    }

    pub fn insert(&mut self, item: T) -> Handle<T> {
        let arena_index = self.next_arena_index();
        self.push(item);

        Handle::from_arena_index(arena_index)
    }

    pub fn insert_many(&mut self, items: impl IntoIterator<Item = T>) -> HandleSpan<T> {
        let start_index = self.next_arena_index();
        let mut count = 0u32;

        for item in items {
            self.push(item);
            count = count
                .checked_add(1)
                .expect("paged arena span count overflow");
        }

        if count == 0 {
            HandleSpan::empty()
        } else {
            HandleSpan::from_parts(Handle::from_arena_index(start_index), count)
        }
    }

    pub fn get(&self, handle: Handle<T>) -> &T {
        let Some((page_index, slot_index)) = self.valid_position(handle) else {
            return self.dummy();
        };

        self.pages
            .get(page_index)
            .and_then(|page| page.get(slot_index))
            .unwrap_or_else(|| self.dummy())
    }

    /// Mutable access to an existing item. Invalid handles are a programmer
    /// error here: unlike read access, a write cannot safely target the
    /// arena's shared dummy value.
    pub fn get_mut(&mut self, handle: Handle<T>) -> &mut T {
        let (page_index, slot_index) = self
            .valid_position(handle)
            .expect("paged arena mutable handle must be valid");
        self.pages
            .get_mut(page_index)
            .and_then(|page| page.get_mut(slot_index))
            .expect("validated paged arena position must exist")
    }

    pub fn span(&self, span: HandleSpan<T>) -> Option<&[T]> {
        if span.is_empty() {
            return Some(&[]);
        }

        let range = self.page_local_range(span)?;

        self.pages.get(range.page_index)?.get(range.slots)
    }

    pub fn span_or_empty(&self, span: HandleSpan<T>) -> &[T] {
        self.span(span).unwrap_or(&[])
    }

    pub fn paged_span(&self, span: HandleSpan<T>) -> Option<PagedSlice<'_, T>> {
        if span.is_empty() {
            return Some(PagedSlice {
                arena: self,
                start_index: 1,
                count: 0,
                marker: PhantomData,
            });
        }

        let start_index = self.valid_logical_index(span.start())?;
        let count = usize::try_from(span.count()).ok()?;
        let end_index = start_index.checked_add(count)?;

        if end_index > self.len {
            return None;
        }

        Some(PagedSlice {
            arena: self,
            start_index,
            count,
            marker: PhantomData,
        })
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn page_size(&self) -> usize {
        self.page_size
    }

    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    pub fn dummy(&self) -> &T {
        &self.dummy
    }

    pub fn iter(&self) -> PagedArenaIter<'_, T> {
        PagedArenaIter {
            arena: self,
            next_index: 0,
            marker: PhantomData,
        }
    }

    pub fn into_items(self) -> impl Iterator<Item = T> {
        self.pages.into_iter().flatten()
    }

    fn push(&mut self, item: T) {
        let needs_page = self
            .pages
            .last()
            .is_none_or(|page| page.len() == self.page_size);

        if needs_page {
            self.pages.push(Vec::with_capacity(self.page_size));
        }

        self.pages
            .last_mut()
            .expect("paged arena should have a page")
            .push(item);
        self.len = self
            .len
            .checked_add(1)
            .expect("paged arena length overflow");
    }

    fn next_arena_index(&self) -> u32 {
        self.len
            .checked_add(1)
            .and_then(|index| u32::try_from(index).ok())
            .expect("paged arena index overflow")
    }

    fn valid_logical_index(&self, handle: Handle<T>) -> Option<usize> {
        if !handle.is_valid() || handle.generation() != 1 {
            return None;
        }

        let logical_index = usize::try_from(handle.arena_index()).ok()?.checked_sub(1)?;

        if logical_index < self.len {
            Some(logical_index)
        } else {
            None
        }
    }

    fn valid_position(&self, handle: Handle<T>) -> Option<(usize, usize)> {
        let logical_index = self.valid_logical_index(handle)?;

        Some(self.position_from_logical_index(logical_index))
    }

    fn position_from_logical_index(&self, logical_index: usize) -> (usize, usize) {
        (
            logical_index / self.page_size,
            logical_index % self.page_size,
        )
    }

    fn page_local_range(&self, span: HandleSpan<T>) -> Option<PageLocalRange> {
        let start_index = self.valid_logical_index(span.start())?;
        let count = usize::try_from(span.count()).ok()?;
        let end_index = start_index.checked_add(count)?;

        if end_index > self.len {
            return None;
        }

        let (page_index, start_slot) = self.position_from_logical_index(start_index);
        let (_, end_slot_exclusive) = self.position_from_logical_index(end_index - 1);

        if start_slot + count > self.page_size {
            return None;
        }

        Some(PageLocalRange {
            page_index,
            slots: start_slot..end_slot_exclusive + 1,
        })
    }
}

impl<T: Default> Default for PagedArena<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PageLocalRange {
    page_index: usize,
    slots: Range<usize>,
}

pub struct PagedSlice<'arena, T> {
    arena: &'arena PagedArena<T>,
    start_index: usize,
    count: usize,
    marker: PhantomData<&'arena T>,
}

impl<'arena, T: Default> PagedSlice<'arena, T> {
    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub fn get(&self, index: usize) -> Option<&'arena T> {
        if index >= self.count {
            return None;
        }

        let logical_index = self.start_index.checked_add(index)?;
        let (page_index, slot_index) = self.arena.position_from_logical_index(logical_index);

        self.arena
            .pages
            .get(page_index)
            .and_then(|page| page.get(slot_index))
    }

    pub fn iter(&self) -> PagedSliceIter<'arena, T> {
        PagedSliceIter {
            slice: PagedSlice {
                arena: self.arena,
                start_index: self.start_index,
                count: self.count,
                marker: PhantomData,
            },
            next_index: 0,
        }
    }
}

pub struct PagedSliceIter<'arena, T> {
    slice: PagedSlice<'arena, T>,
    next_index: usize,
}

impl<'arena, T: Default> Iterator for PagedSliceIter<'arena, T> {
    type Item = &'arena T;

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.slice.get(self.next_index)?;
        self.next_index += 1;

        Some(item)
    }
}

pub struct PagedArenaIter<'arena, T> {
    arena: &'arena PagedArena<T>,
    next_index: usize,
    marker: PhantomData<&'arena T>,
}

impl<'arena, T: Default> Iterator for PagedArenaIter<'arena, T> {
    type Item = (Handle<T>, &'arena T);

    fn next(&mut self) -> Option<Self::Item> {
        if self.next_index >= self.arena.len {
            return None;
        }

        let arena_index = self
            .next_index
            .checked_add(1)
            .and_then(|index| u32::try_from(index).ok())
            .expect("paged arena index overflow");
        let handle = Handle::from_arena_index(arena_index);
        let item = self.arena.get(handle);
        self.next_index += 1;

        Some((handle, item))
    }
}

#[cfg(test)]
mod tests {
    use crate::{Handle, PagedArena};

    #[test]
    fn resolves_invalid_handles_to_dummy() {
        let mut arena = PagedArena::<String>::with_page_size(4);
        let handle = arena.insert("alpha".to_owned());
        let invalid = Handle::<String>::invalid();

        assert_eq!(handle.arena_index(), 1);
        assert_eq!(arena.get(invalid), "");
        assert_eq!(arena.get(Handle::from_parts(handle.arena_index(), 2)), "");
    }

    #[test]
    fn appends_across_pages() {
        let mut arena = PagedArena::<String>::with_page_size(2);
        let first = arena.insert("alpha".to_owned());
        let second = arena.insert("beta".to_owned());
        let third = arena.insert("gamma".to_owned());

        assert_eq!(arena.page_count(), 2);
        assert_eq!(arena.get(first), "alpha");
        assert_eq!(arena.get(second), "beta");
        assert_eq!(arena.get(third), "gamma");
    }

    #[test]
    fn page_local_span_returns_slice() {
        let mut arena = PagedArena::<String>::with_page_size(4);
        let span = arena.insert_many(["alpha".to_owned(), "beta".to_owned()]);

        assert_eq!(
            arena.span(span).expect("span should be page-local"),
            &["alpha".to_owned(), "beta".to_owned()]
        );
    }

    #[test]
    fn cross_page_span_returns_paged_slice() {
        let mut arena = PagedArena::<String>::with_page_size(2);
        let span = arena.insert_many(["alpha".to_owned(), "beta".to_owned(), "gamma".to_owned()]);

        assert!(arena.span(span).is_none());
        let values = arena
            .paged_span(span)
            .expect("paged span should resolve")
            .iter()
            .cloned()
            .collect::<Vec<_>>();

        assert_eq!(
            values,
            vec!["alpha".to_owned(), "beta".to_owned(), "gamma".to_owned()]
        );
    }

    #[test]
    fn iterates_with_stable_handles() {
        let mut arena = PagedArena::<String>::with_page_size(2);
        arena.insert("alpha".to_owned());
        arena.insert("beta".to_owned());
        arena.insert("gamma".to_owned());

        let entries = arena
            .iter()
            .map(|(handle, value)| (handle.arena_index(), value.clone()))
            .collect::<Vec<_>>();

        assert_eq!(
            entries,
            vec![
                (1, "alpha".to_owned()),
                (2, "beta".to_owned()),
                (3, "gamma".to_owned())
            ]
        );
    }
}
