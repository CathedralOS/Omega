//! Concurrent construction over worker-owned pages.
//!
//! A worker reserves one page identity, builds an ordinary `Vec<T>` privately,
//! and publishes that allocation into the shared directory. Handles use their
//! generation coordinate as the page identity and their arena-index coordinate
//! as the one-based slot, so every [`HandleSpan`] remains page-local and resolves
//! to one ordinary slice. Sealing consumes the construction owner after worker
//! borrows end; it does not copy nodes, flatten pages, or build a lookup table.

use std::marker::PhantomData;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use crate::{Handle, HandleSpan};

const PAGES_PER_DIRECTORY_SEGMENT: usize = 256;

/// Concurrent page publication before immutable compiler consumption.
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
    /// Create a bounded page directory. Pages allocate their payload only when
    /// a worker publishes them; only the top-level segment directory scales
    /// with `maximum_pages`, at one empty cell per 256 possible pages.
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

    /// Reserve one page identity for a worker-owned allocation.
    pub fn reserve_page(&self) -> Option<ConcurrentArenaPage<'_, T>> {
        self.reserve_page_with_capacity(self.default_page_capacity)
    }

    /// Reserve one page identity with an explicit initial vector capacity.
    /// The page may grow normally while its worker owns it.
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

    /// Seal the exact published pages without moving their values. A dropped
    /// or panicked reservation makes the construction incomplete and therefore
    /// unpublishable.
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

/// One page identity and its worker-private final allocation.
pub struct ConcurrentArenaPage<'arena, T: Default> {
    arena: &'arena ConcurrentPagedArena<T>,
    page_index: u32,
    values: Vec<T>,
}

impl<T: Default> ConcurrentArenaPage<'_, T> {
    /// Nonzero identity carried in the generation coordinate of every handle
    /// whose payload this page owns.
    pub const fn page_identity(&self) -> u32 {
        self.arena.first_page_identity + self.page_index
    }

    /// Append one value and return its final page-and-slot handle.
    pub fn push(&mut self, value: T) -> Handle<T> {
        let handle = self.next_handle();
        self.values.push(value);
        handle
    }

    /// Append one page-local range. Worker-local collection remains scratch;
    /// these values land directly in the page allocation that is published.
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

    /// Read a value already constructed on this private page.
    pub fn get(&self, handle: Handle<T>) -> Option<&T> {
        self.local_slot(handle)
            .and_then(|slot| self.values.get(slot))
    }

    /// Mutate a value already constructed on this private page. Publication
    /// removes this operation by consuming the page owner.
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

    /// Publish this exact vector allocation into the shared directory.
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

/// The identity and populated length of one published page.
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

/// Paged storage after its concurrent page set is sealed. The page directory
/// no longer grows; an exclusive owner may still perform later compiler-stage
/// rewrites through ordinary mutable borrows.
pub struct SealedPagedArena<T: Default> {
    directory: PageDirectory<T>,
    first_page_identity: u32,
    page_count: u32,
    value_count: u64,
    dummy: T,
}

impl<T: Default> SealedPagedArena<T> {
    /// Begin another concurrent append epoch over the same published pages.
    /// Existing handles and allocations stay unchanged; new reservations start
    /// after the sealed page frontier.
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

    /// Resolve a page-local span to an ordinary contiguous slice.
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

    /// Mutate one value after concurrent construction has ended.
    pub fn get_mut(&mut self, handle: Handle<T>) -> Option<&mut T> {
        let page_index = self.page_index(handle)?;
        let slot_index = self.slot_index(handle)?;
        self.directory
            .page_mut(page_index)?
            .get_mut()?
            .get_mut(slot_index)
    }

    /// Mutate one page-local span after concurrent construction has ended.
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

    /// Read the exact allocation a worker published for `page`.
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

    /// Consume the sealed directory into its worker-published vectors in page
    /// identity order. Values remain in their original allocations.
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

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Barrier};

    use super::ConcurrentPagedArena;
    use crate::Handle;

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
}
