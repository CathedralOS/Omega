use std::marker::PhantomData;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use arc_swap::ArcSwapOption;

use crate::arena::Handle;
use crate::arena::free_stack::FreeStack;

const SLOT_EMPTY: u8 = 0;
const SLOT_ACTIVE: u8 = 1;
const SLOT_FREED: u8 = 2;

const PAGE_SIZE: usize = 16;

pub struct PagedArena<T: Default> {
    pages: Box<[ArcSwapOption<Page<T>>]>,
    slot_metadata: Box<[AtomicU64]>,
    dummy_page: Arc<Page<T>>,
    free_stack: FreeStack,
}

impl<T: Default> PagedArena<T> {
    pub fn new(max_capacity: usize) -> Self {
        assert!(
            max_capacity >= PAGE_SIZE,
            "paged arena max capacity must be at least {PAGE_SIZE}"
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

        let Some(page) = self.load_or_alloc_page(page_index) else {
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

    pub fn reclaim_empty_pages(&self) -> usize {
        let mut reclaimed = 0;
        let mut kept_headroom = false;

        for page_index in 1..self.pages.len() {
            if self.pages[page_index].load().is_none() || !self.page_is_inactive(page_index) {
                continue;
            }

            if !kept_headroom {
                kept_headroom = true;
                continue;
            }

            self.pages[page_index].store(None);
            reclaimed += 1;
        }

        reclaimed
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
                .expect("paged arena generation overflow");
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

    fn page_is_inactive(&self, page_index: usize) -> bool {
        let start_index = page_index * PAGE_SIZE;
        let end_index = start_index + PAGE_SIZE;

        self.slot_metadata[start_index..end_index]
            .iter()
            .all(|slot_metadata| {
                let (state, _) = unpack_slot(slot_metadata.load(Ordering::Relaxed));
                state != SLOT_ACTIVE
            })
    }

    fn compose(&self, page_index: usize, slot_index: usize) -> u32 {
        u32::try_from(page_index * PAGE_SIZE + slot_index).expect("paged arena index overflow")
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
