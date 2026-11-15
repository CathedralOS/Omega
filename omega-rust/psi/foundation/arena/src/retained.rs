//! Retained tables keep their published rows while a continuation appends new
//! rows in its own page. Handles keep their absolute slot and generation;
//! retaining a page never renumbers a stored reference. A span issued by this
//! table belongs to one page, including when an old span is extended.

use std::sync::Arc;

use crate::arena::{RevisionStamp, refuse_unresolved, refuse_unresolved_span};
use crate::{Arena, Handle, HandleSpan};

#[derive(Debug, Clone, PartialEq, Eq)]
struct RetainedPage<T> {
    start: usize,
    rows: Arc<Arena<T>>,
}

/// Absolute-slot tables whose continuations share retained rows and own their
/// appends. Rewriting an existing row detaches its page, not the append output
/// of other continuations. Retired slots are not reused.
///
/// Issued spans always fit in one page. A caller must extend a span through
/// `append_to_span`, not manufacture a span across retained and appended rows.
#[derive(Debug, Clone)]
pub struct RetainedArena<T> {
    pages: Vec<RetainedPage<T>>,
    appended: Arc<Arena<T>>,
    start: usize,
    revision: RevisionStamp,
    rewrite_revision: u64,
    append_only: bool,
    retired: usize,
}

impl<T: Default> Default for RetainedArena<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Default> RetainedArena<T> {
    pub fn new() -> Self {
        Self {
            pages: Vec::new(),
            appended: Arc::new(Arena::new()),
            start: 0,
            revision: RevisionStamp::unstamped(),
            rewrite_revision: 0,
            append_only: true,
            retired: 0,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        let mut rows = Self::new();
        rows.reserve(capacity);
        rows
    }

    /// An append after a fork starts a private page, rather than making an
    /// independent copy of the retained rows. Read-only forks allocate no rows.
    fn prepare_append(&mut self) {
        if Arc::strong_count(&self.appended) == 1 {
            return;
        }
        let length = self.appended.storage_slice().len();
        let generation = self.appended.storage_view().fresh_generation;
        if length == 0 {
            self.appended = Arc::new(Arena::with_generation(generation));
            return;
        }
        self.pages.push(RetainedPage {
            start: self.start,
            rows: std::mem::replace(
                &mut self.appended,
                Arc::new(Arena::with_generation(generation)),
            ),
        });
        self.start = self
            .start
            .checked_add(length)
            .expect("retained arena slot overflow");
    }

    fn storage_len(&self) -> usize {
        self.start + self.appended.storage_slice().len()
    }

    fn page_position(&self, slot: usize) -> Option<usize> {
        self.pages
            .partition_point(|page| page.start <= slot)
            .checked_sub(1)
    }

    fn local(handle: Handle<T>, start: usize) -> Handle<T> {
        Handle::from_parts(
            handle
                .arena_index()
                .saturating_sub(u32::try_from(start).expect("retained arena slot overflow")),
            handle.generation(),
        )
    }

    fn absolute(handle: Handle<T>, start: usize) -> Handle<T> {
        if !handle.is_valid() {
            return handle;
        }
        Handle::from_parts(
            handle
                .arena_index()
                .checked_add(u32::try_from(start).expect("retained arena slot overflow"))
                .expect("retained arena slot overflow"),
            handle.generation(),
        )
    }

    fn absolute_span(span: HandleSpan<T>, start: usize) -> HandleSpan<T> {
        HandleSpan::from_parts(Self::absolute(span.start(), start), span.count())
    }

    fn local_span(span: HandleSpan<T>, start: usize) -> HandleSpan<T> {
        HandleSpan::from_parts(Self::local(span.start(), start), span.count())
    }

    fn page(&self, handle: Handle<T>) -> Option<(usize, &Arena<T>)> {
        let slot = usize::try_from(handle.arena_index().checked_sub(1)?).ok()?;
        if slot >= self.start {
            return Some((self.start, &self.appended));
        }
        let page = &self.pages[self.page_position(slot)?];
        Some((page.start, &page.rows))
    }

    pub fn revision(&self) -> u64 {
        self.revision.get()
    }

    pub fn append_epoch(&self) -> Option<(u64, usize)> {
        self.append_only
            .then_some((self.rewrite_revision, self.storage_len()))
    }

    pub fn reserve(&mut self, additional: usize) {
        self.prepare_append();
        Arc::get_mut(&mut self.appended)
            .expect("private append page")
            .reserve(additional);
    }

    pub fn insert(&mut self, value: T) -> Handle<T> {
        self.append(value)
    }

    pub fn append(&mut self, value: T) -> Handle<T> {
        self.revision.invalidate();
        self.prepare_append();
        Self::absolute(
            Arc::get_mut(&mut self.appended)
                .expect("private append page")
                .append(value),
            self.start,
        )
    }

    pub fn insert_many(&mut self, values: impl IntoIterator<Item = T>) -> HandleSpan<T> {
        self.revision.invalidate();
        self.prepare_append();
        Self::absolute_span(
            Arc::get_mut(&mut self.appended)
                .expect("private append page")
                .insert_many(values),
            self.start,
        )
    }

    pub fn try_insert_many<E>(
        &mut self,
        values: impl IntoIterator<Item = Result<T, E>>,
    ) -> Result<HandleSpan<T>, E> {
        self.revision.invalidate();
        self.prepare_append();
        Arc::get_mut(&mut self.appended)
            .expect("private append page")
            .try_insert_many(values)
            .map(|span| Self::absolute_span(span, self.start))
    }

    pub fn try_get(&self, handle: Handle<T>) -> Option<&T> {
        let (start, page) = self.page(handle)?;
        page.try_get(Self::local(handle, start))
    }

    #[track_caller]
    pub fn get(&self, handle: Handle<T>) -> &T {
        if !handle.is_valid() {
            return self.appended.dummy();
        }
        self.try_get(handle)
            .unwrap_or_else(|| refuse_unresolved("get", handle.arena_index(), handle.generation()))
    }

    pub fn is_valid(&self, handle: Handle<T>) -> bool {
        self.try_get(handle).is_some()
    }

    pub fn span(&self, span: HandleSpan<T>) -> Option<&[T]> {
        if span.is_empty() {
            return Some(&[]);
        }
        let (start, page) = self.page(span.start())?;
        page.span(Self::local_span(span, start))
    }

    #[track_caller]
    pub fn span_or_empty(&self, span: HandleSpan<T>) -> &[T] {
        self.span(span)
            .unwrap_or_else(|| refuse_unresolved_span("span_or_empty", span))
    }

    pub fn len(&self) -> usize {
        self.storage_len() - self.retired
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn iter(&self) -> impl Iterator<Item = (Handle<T>, &T)> {
        self.iter_from(0)
    }

    pub fn iter_from(&self, slot: usize) -> impl Iterator<Item = (Handle<T>, &T)> {
        let first_page = self
            .pages
            .partition_point(|page| page.start + page.rows.storage_slice().len() <= slot);
        self.pages[first_page..]
            .iter()
            .map(|page| (page.start, page.rows.as_ref()))
            .chain(std::iter::once((self.start, self.appended.as_ref())))
            .filter(move |(start, page)| *start + page.storage_slice().len() > slot)
            .flat_map(move |(start, page)| {
                page.iter_from(slot.saturating_sub(start))
                    .map(move |(handle, row)| (Self::absolute(handle, start), row))
            })
    }

    /// Find a partition in stored rows without flattening retained pages.
    /// As with a slice partition, the caller supplies a partitioned predicate;
    /// retired slots still occupy their original storage positions.
    pub fn partition_point(&self, mut predicate: impl FnMut(&T) -> bool) -> usize {
        let page = self.pages.partition_point(|page| {
            predicate(
                page.rows
                    .storage_slice()
                    .last()
                    .expect("retained page is nonempty"),
            )
        });
        match self.pages.get(page) {
            Some(page) => page.start + page.rows.storage_slice().partition_point(predicate),
            None => self.start + self.appended.storage_slice().partition_point(predicate),
        }
    }

    fn stored_rows(&self) -> impl Iterator<Item = (&T, u32, bool)> {
        self.pages
            .iter()
            .map(|page| page.rows.as_ref())
            .chain(std::iter::once(self.appended.as_ref()))
            .flat_map(|page| {
                let storage = page.storage_view();
                storage.items.iter().enumerate().map(move |(slot, row)| {
                    let generation = storage
                        .slots
                        .map_or(storage.fresh_generation, |slots| slots.generations[slot]);
                    let occupied = storage.slots.is_none_or(|slots| slots.occupied[slot]);
                    (row, generation, occupied)
                })
            })
    }
}

impl<T: Default + Clone> RetainedArena<T> {
    fn rewrite(&mut self) {
        self.revision.invalidate();
        self.rewrite_revision = self
            .rewrite_revision
            .checked_add(1)
            .expect("retained arena revision overflow");
    }

    fn page_mut(&mut self, handle: Handle<T>) -> Option<(usize, &mut Arena<T>)> {
        let slot = usize::try_from(handle.arena_index().checked_sub(1)?).ok()?;
        if slot >= self.start {
            return Some((self.start, Arc::make_mut(&mut self.appended)));
        }
        let position = self.page_position(slot)?;
        let page = &mut self.pages[position];
        Some((page.start, Arc::make_mut(&mut page.rows)))
    }

    pub fn reset_retain_capacity(&mut self) {
        self.rewrite();
        if let Some(appended) = Arc::get_mut(&mut self.appended) {
            appended.reset_retain_capacity();
        } else {
            let generation = self.appended.storage_view().fresh_generation;
            let dummy = self.appended.dummy().clone();
            self.appended = Arc::new(
                Arena::from_storage(dummy, Vec::new(), None, generation, true)
                    .expect("empty retained append page is valid"),
            );
        }
        let mut previous_pages = std::mem::take(&mut self.pages);
        self.start = 0;
        self.retired = 0;
        self.append_only = true;
        previous_pages.clear();
        self.pages = previous_pages;
    }

    #[track_caller]
    pub fn get_mut(&mut self, handle: Handle<T>) -> &mut T {
        if !self.is_valid(handle) {
            refuse_unresolved("get_mut", handle.arena_index(), handle.generation());
        }
        self.rewrite();
        let (start, page) = self
            .page_mut(handle)
            .expect("validated retained arena handle");
        page.get_mut(Self::local(handle, start))
    }

    pub fn free(&mut self, handle: Handle<T>) -> bool {
        if !self.is_valid(handle) {
            return false;
        }
        self.rewrite();
        let (start, page) = self
            .page_mut(handle)
            .expect("validated retained arena handle");
        let retired = page.free(Self::local(handle, start));
        if retired {
            self.append_only = false;
            self.retired += 1;
        }
        retired
    }

    pub fn span_mut(&mut self, span: HandleSpan<T>) -> Option<&mut [T]> {
        if span.is_empty() {
            return Some(&mut []);
        }
        self.span(span)?;
        self.rewrite();
        let (start, page) = self.page_mut(span.start())?;
        page.span_mut(Self::local_span(span, start))
    }

    #[track_caller]
    pub fn span_mut_or_empty(&mut self, span: HandleSpan<T>) -> &mut [T] {
        self.span_mut(span)
            .unwrap_or_else(|| refuse_unresolved_span("span_mut_or_empty", span))
    }

    pub fn copy_span_pair(&mut self, first: HandleSpan<T>, second: HandleSpan<T>) -> HandleSpan<T> {
        let values = self
            .span_or_empty(first)
            .iter()
            .chain(self.span_or_empty(second))
            .cloned()
            .collect::<Vec<_>>();
        self.insert_many(values)
    }

    pub fn append_to_span(&mut self, span: &mut HandleSpan<T>, value: T) -> Handle<T> {
        self.prepare_append();
        if !span.is_empty() && span.start().arena_index() as usize <= self.start {
            *span = self.copy_span_pair(*span, HandleSpan::empty());
        }
        let mut local = Self::local_span(*span, self.start);
        let handle = Arc::get_mut(&mut self.appended)
            .expect("private append page")
            .append_to_span(&mut local, value);
        self.revision.invalidate();
        *span = Self::absolute_span(local, self.start);
        Self::absolute(handle, self.start)
    }
}

impl<T: compiler_storage::Record + 'static> compiler_storage::Record for RetainedPage<T> {
    fn write(
        &self,
        output: &mut compiler_storage::RecordWriter,
    ) -> Result<(), compiler_storage::RecordError> {
        self.start.write(output)?;
        self.rows.write(output)
    }

    fn read<Context>(
        input: &mut compiler_storage::RecordReader<'_, Context>,
    ) -> Result<Self, compiler_storage::RecordError> {
        Ok(Self {
            start: usize::read(input)?,
            rows: Arc::read(input)?,
        })
    }
}

impl<T: Default + compiler_storage::Record + 'static> compiler_storage::Record
    for RetainedArena<T>
{
    fn write(
        &self,
        output: &mut compiler_storage::RecordWriter,
    ) -> Result<(), compiler_storage::RecordError> {
        output.record(|output| {
            self.pages.write(output)?;
            self.appended.write(output)?;
            self.start.write(output)
        })
    }

    fn read<Context>(
        input: &mut compiler_storage::RecordReader<'_, Context>,
    ) -> Result<Self, compiler_storage::RecordError> {
        input.record(|input| {
            let pages = Vec::<RetainedPage<T>>::read(input)?;
            let appended = Arc::<Arena<T>>::read(input)?;
            let start = usize::read(input)?;
            let mut expected = 0usize;
            for page in &pages {
                if page.start != expected || page.rows.storage_slice().is_empty() {
                    return Err(input.error("retained arena pages have invalid slot ranges"));
                }
                expected = expected
                    .checked_add(page.rows.storage_slice().len())
                    .ok_or_else(|| input.error("retained arena slot overflow"))?;
            }
            if start != expected
                || start
                    .checked_add(appended.storage_slice().len())
                    .is_none_or(|length| length > u32::MAX as usize)
            {
                return Err(input.error("retained arena append range is invalid"));
            }
            let append_only = pages.iter().all(|page| page.rows.append_epoch().is_some())
                && appended.append_epoch().is_some();
            let retired = pages
                .iter()
                .map(|page| page.rows.storage_slice().len() - page.rows.len())
                .sum::<usize>()
                + appended.storage_slice().len()
                - appended.len();
            Ok(Self {
                pages,
                appended,
                start,
                revision: RevisionStamp::unstamped(),
                rewrite_revision: 0,
                append_only,
                retired,
            })
        })
    }
}

impl<T: Default + PartialEq> PartialEq for RetainedArena<T> {
    fn eq(&self, other: &Self) -> bool {
        self.appended.dummy() == other.appended.dummy()
            // Equal stored rows must also produce equal future handles, even
            // when no live row records the append generation.
            && self.appended.storage_view().fresh_generation
                == other.appended.storage_view().fresh_generation
            && self.storage_len() == other.storage_len()
            && self.stored_rows().eq(other.stored_rows())
    }
}

impl<T: Default + Eq> Eq for RetainedArena<T> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cloning_and_appending_do_not_clone_retained_payloads() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        #[derive(Default)]
        struct Tracked(Arc<AtomicUsize>);

        impl Clone for Tracked {
            fn clone(&self) -> Self {
                self.0.fetch_add(1, Ordering::Relaxed);
                Self(Arc::clone(&self.0))
            }
        }

        let copies = Arc::new(AtomicUsize::new(0));
        let mut original = RetainedArena::new();
        original.insert_many((0..100).map(|_| Tracked(Arc::clone(&copies))));
        let mut cleared = original.clone();
        cleared.reset_retain_capacity();
        assert_eq!(cleared.len(), 0);
        assert_eq!(copies.load(Ordering::Relaxed), 0);
        let mut selected = original.clone();
        selected.append(Tracked(Arc::clone(&copies)));
        assert_eq!(copies.load(Ordering::Relaxed), 0);
        let mut span = HandleSpan::from_parts(Handle::from_arena_index(10), 2);
        selected.append_to_span(&mut span, Tracked(Arc::clone(&copies)));
        assert_eq!(copies.load(Ordering::Relaxed), 2);
        assert_eq!(original.len(), 100);
        assert_eq!(selected.len(), 104);
    }

    #[test]
    fn continuation_shares_rows_and_owns_new_output() {
        let mut original = RetainedArena::<String>::new();
        let retained = original.insert_many(["first".into(), "second".into()]);
        let mut selected = original.clone();
        assert!(std::ptr::eq(
            original.get(retained.start()),
            selected.get(retained.start())
        ));
        let added = selected.insert_many(["third".into(), "fourth".into()]);
        assert_eq!(added.start().arena_index(), 3);
        assert_eq!(added.start().generation(), retained.start().generation());
        assert_eq!(selected.span_or_empty(retained), ["first", "second"]);
        assert_eq!(selected.span_or_empty(added), ["third", "fourth"]);
        assert!(!original.is_valid(added.start()));
        assert!(std::ptr::eq(
            original.get(retained.start()),
            selected.get(retained.start())
        ));
        selected.get_mut(retained.start()).push_str(" changed");
        assert_eq!(original.get(retained.start()), "first");
        assert_eq!(selected.get(retained.start()), "first changed");
        let stale = Handle::from_parts(
            retained.start().arena_index(),
            retained.start().generation() + 1,
        );
        assert!(!selected.is_valid(stale));
        assert!(
            selected
                .span(HandleSpan::from_parts(retained.start(), 3))
                .is_none()
        );
    }

    #[test]
    fn extending_a_retained_span_copies_only_that_span() {
        let mut original = RetainedArena::<u32>::new();
        original.insert_many([100, 200]);
        let mut span = original.insert_many([1, 2]);
        let mut selected = original.clone();
        let last = selected.append_to_span(&mut span, 3);
        assert_eq!(selected.span_or_empty(span), [1, 2, 3]);
        assert_eq!(last.arena_index(), 7);
        assert_eq!(
            original.iter().map(|(_, row)| *row).collect::<Vec<_>>(),
            [100, 200, 1, 2]
        );
        assert!(std::ptr::eq(
            original.get(Handle::from_arena_index(1)),
            selected.get(Handle::from_arena_index(1))
        ));
    }

    #[test]
    fn rows_compare_independently_of_continuation_pages() {
        let mut first = RetainedArena::<u32>::new();
        first.insert_many([1, 2]);
        let mut second = first.clone();
        first.insert_many([3, 4]);
        second.insert_many([3, 4]);
        let mut contiguous = RetainedArena::<u32>::new();
        contiguous.insert_many([1, 2, 3, 4]);
        assert_eq!(first, second);
        assert_eq!(first, contiguous);
        assert_eq!(
            first
                .iter_from(2)
                .map(|(handle, value)| (handle.arena_index(), *value))
                .collect::<Vec<_>>(),
            [(3, 3), (4, 4)]
        );
        assert!(first.free(Handle::from_arena_index(1)));
        assert!(!first.is_valid(Handle::from_arena_index(1)));
        assert!(!first.free(Handle::from_arena_index(1)));
        assert_eq!(first.append_epoch(), None);
        assert!(second.is_valid(Handle::from_arena_index(1)));
        assert_eq!(first.append(5).arena_index(), 5);
    }

    #[test]
    fn partition_queries_and_reset_keep_retained_pages_independent() {
        let mut original = RetainedArena::with_capacity(8);
        original.insert_many([1u32, 2, 3]);
        let mut selected = original.clone();
        selected.insert_many([4, 5]);
        for threshold in 0..=6 {
            assert_eq!(
                selected.partition_point(|value| *value < threshold),
                [1u32, 2, 3, 4, 5].partition_point(|value| *value < threshold)
            );
        }
        selected.reset_retain_capacity();
        assert!(selected.is_empty());
        assert_eq!(selected.append(9).arena_index(), 1);
        assert_eq!(
            original.iter().map(|(_, value)| *value).collect::<Vec<_>>(),
            [1, 2, 3]
        );
    }

    #[test]
    fn private_record_keeps_page_ranges_and_generations() {
        let mut first = RetainedArena::<u32>::new();
        let retained = first.insert_many([1, 2]);
        let mut selected = first.clone();
        let added = selected.insert_many([3, 4]);
        selected.free(retained.start());
        let limits = compiler_storage::RecordLimits::default();
        let bytes = compiler_storage::encode(&selected, limits).unwrap();
        let decoded: RetainedArena<u32> = compiler_storage::decode(&bytes, limits).unwrap();
        assert_eq!(selected, decoded);
        assert!(!decoded.is_valid(retained.start()));
        assert_eq!(decoded.span_or_empty(added), [3, 4]);
    }

    #[test]
    fn fallible_append_and_nondefault_generation_keep_retained_coordinates() {
        let mut original = RetainedArena::<u32>::new();
        original.appended = Arc::new(Arena::with_generation(9));
        let retained = original.append(1);
        let mut selected = original.clone();
        assert_eq!(selected.try_insert_many([Ok(2), Err("stop")]), Err("stop"));
        assert_eq!(selected.len(), 1);
        assert!(std::ptr::eq(original.get(retained), selected.get(retained)));
        let added = selected.append(2);
        assert_eq!(added.arena_index(), 2);
        assert_eq!(added.generation(), 9);
        assert!(selected.try_get(Handle::from_arena_index(2)).is_none());
        assert_eq!(
            selected.iter().map(|(_, row)| *row).collect::<Vec<_>>(),
            [1, 2]
        );
    }

    #[test]
    fn private_record_refuses_overlapping_page_ranges() {
        let mut original = RetainedArena::<u32>::new();
        original.append(1);
        let mut selected = original.clone();
        selected.append(2);
        selected.pages[0].start = 1;
        let limits = compiler_storage::RecordLimits::default();
        let bytes = compiler_storage::encode(&selected, limits).unwrap();
        assert!(compiler_storage::decode::<RetainedArena<u32>>(&bytes, limits).is_err());
    }

    #[test]
    fn failed_page_detachment_does_not_retire_a_live_row() {
        use std::sync::atomic::{AtomicBool, Ordering};

        #[derive(Default)]
        struct PanickingClone(Arc<AtomicBool>);

        impl Clone for PanickingClone {
            fn clone(&self) -> Self {
                assert!(!self.0.load(Ordering::Relaxed), "payload clone failed");
                Self(Arc::clone(&self.0))
            }
        }

        let should_panic = Arc::new(AtomicBool::new(false));
        let mut original = RetainedArena::new();
        let handle = original.append(PanickingClone(Arc::clone(&should_panic)));
        let mut selected = original.clone();
        should_panic.store(true, Ordering::Relaxed);
        let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            selected.free(handle);
        }));
        assert!(failed.is_err());
        assert_eq!(selected.len(), 1);
        assert!(selected.is_valid(handle));
        should_panic.store(false, Ordering::Relaxed);
        assert!(selected.free(handle));
        assert_eq!(selected.len(), 0);
        assert_eq!(original.len(), 1);
        assert!(original.is_valid(handle));
    }

    #[test]
    fn equality_keeps_the_generation_used_by_the_next_append() {
        for retire_existing_row in [false, true] {
            let mut original = RetainedArena::<u32>::new();
            if retire_existing_row {
                let existing = original.append(1);
                assert!(original.free(existing));
            }
            let storage = original.appended.storage_view();
            let mut other_generation = original.clone();
            other_generation.appended = Arc::new(
                Arena::from_storage(
                    *storage.dummy,
                    storage.items.to_vec(),
                    storage.slots.cloned(),
                    9,
                    storage.fresh,
                )
                .unwrap(),
            );
            let original_next = original.clone().append(2);
            let other_next = other_generation.clone().append(2);
            assert_eq!(original_next.arena_index(), other_next.arena_index());
            assert_ne!(original_next.generation(), other_next.generation());
            assert_ne!(original, other_generation);
        }
    }

    #[test]
    #[should_panic(expected = "get_mut")]
    fn unresolved_mutation_never_changes_the_default_row() {
        RetainedArena::<u32>::new().get_mut(Handle::invalid());
    }
}
