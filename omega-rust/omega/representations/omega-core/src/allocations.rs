use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering};

static ALLOCATION_CALLS: AtomicU64 = AtomicU64::new(0);
static DEALLOCATION_CALLS: AtomicU64 = AtomicU64::new(0);
static ALLOCATED_BYTES: AtomicU64 = AtomicU64::new(0);
static DEALLOCATED_BYTES: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AllocationSnapshot {
    pub allocation_calls: u64,
    pub deallocation_calls: u64,
    pub allocated_bytes: u64,
    pub deallocated_bytes: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AllocationDelta {
    pub allocation_calls: u64,
    pub deallocation_calls: u64,
    pub allocated_bytes: u64,
    pub deallocated_bytes: u64,
}

impl AllocationSnapshot {
    pub fn delta_since(self, earlier: Self) -> AllocationDelta {
        AllocationDelta {
            allocation_calls: self
                .allocation_calls
                .saturating_sub(earlier.allocation_calls),
            deallocation_calls: self
                .deallocation_calls
                .saturating_sub(earlier.deallocation_calls),
            allocated_bytes: self.allocated_bytes.saturating_sub(earlier.allocated_bytes),
            deallocated_bytes: self
                .deallocated_bytes
                .saturating_sub(earlier.deallocated_bytes),
        }
    }
}

impl AllocationDelta {
    pub fn net_live_bytes(self) -> i128 {
        i128::from(self.allocated_bytes) - i128::from(self.deallocated_bytes)
    }
}

pub fn snapshot() -> AllocationSnapshot {
    AllocationSnapshot {
        allocation_calls: ALLOCATION_CALLS.load(Ordering::Relaxed),
        deallocation_calls: DEALLOCATION_CALLS.load(Ordering::Relaxed),
        allocated_bytes: ALLOCATED_BYTES.load(Ordering::Relaxed),
        deallocated_bytes: DEALLOCATED_BYTES.load(Ordering::Relaxed),
    }
}

pub struct CountingAllocator<A = System> {
    inner: A,
}

impl CountingAllocator<System> {
    pub const fn system() -> Self {
        Self { inner: System }
    }
}

unsafe impl<A: GlobalAlloc> GlobalAlloc for CountingAllocator<A> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { self.inner.alloc(layout) };

        if !pointer.is_null() {
            ALLOCATION_CALLS.fetch_add(1, Ordering::Relaxed);
            ALLOCATED_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        }

        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe {
            self.inner.dealloc(pointer, layout);
        }

        DEALLOCATION_CALLS.fetch_add(1, Ordering::Relaxed);
        DEALLOCATED_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { self.inner.alloc_zeroed(layout) };

        if !pointer.is_null() {
            ALLOCATION_CALLS.fetch_add(1, Ordering::Relaxed);
            ALLOCATED_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        }

        pointer
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_pointer = unsafe { self.inner.realloc(pointer, layout, new_size) };

        if !new_pointer.is_null() {
            DEALLOCATION_CALLS.fetch_add(1, Ordering::Relaxed);
            DEALLOCATED_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
            ALLOCATION_CALLS.fetch_add(1, Ordering::Relaxed);
            ALLOCATED_BYTES.fetch_add(new_size as u64, Ordering::Relaxed);
        }

        new_pointer
    }
}
