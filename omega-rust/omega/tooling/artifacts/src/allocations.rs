//! Process-wide allocation instrumentation for compiler phase reports.
//! The `omega` binary installs the wrapper; this reporting owner supplies both
//! its counters and the snapshot/delta vocabulary consumed by phase timing.
//!
//! Four `AtomicU64` statics that only ever go up, and `Ordering::Relaxed` on
//! every touch. We are not synchronizing anything with these numbers, we are
//! counting, and a stronger ordering on every `malloc` in a compiler that
//! allocates constantly would land in the very timings this exists to feed.
//! What we give up is that a `snapshot()` taken while other threads allocate is
//! not one consistent instant — the four fields can come from slightly
//! different moments. That is why the useful type is `AllocationDelta` from
//! `delta_since` rather than the snapshot itself: across a phase lasting
//! milliseconds the skew is a handful of allocations against millions.
//!
//! `realloc` counts as a deallocation of the old size plus an allocation of the
//! new one, so `allocation_calls` is larger than the number of distinct objects
//! a phase created, by exactly its reallocation count. A growing `Vec` shows up
//! here once per growth.
//!
//! `delta_since` saturates instead of wrapping, so a snapshot pair passed in
//! the wrong order reads as zero rather than as eighteen quintillion bytes.
//! `net_live_bytes` widens to `i128` because a phase that frees more than it
//! allocated — it inherited the memory from an earlier phase — is ordinary, and
//! the honest answer there is negative.
//!
//! @Robustness: none of this counts anything unless a binary installs the
//! wrapper. `omega/src/command.rs` does, with `#[global_allocator] static
//! GLOBAL_ALLOCATOR: CountingAllocator = CountingAllocator::system();`. Under a
//! test binary, a bench harness, or any other host that does not use the wrapper,
//! every snapshot reads zero
//! and the phase report shows zero bytes allocated — which reads as a
//! measurement rather than as an absent one. There is nothing in the API that
//! lets a caller tell the two apart.

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_deltas_preserve_deallocation_and_reversed_snapshot_behavior() {
        let earlier = AllocationSnapshot {
            allocation_calls: 4,
            deallocation_calls: 2,
            allocated_bytes: 256,
            deallocated_bytes: 32,
        };
        let later = AllocationSnapshot {
            allocation_calls: 5,
            deallocation_calls: 4,
            allocated_bytes: 272,
            deallocated_bytes: 128,
        };
        assert_eq!(
            later.delta_since(earlier),
            AllocationDelta {
                allocation_calls: 1,
                deallocation_calls: 2,
                allocated_bytes: 16,
                deallocated_bytes: 96,
            }
        );
        assert_eq!(later.delta_since(earlier).net_live_bytes(), -80);
        assert_eq!(earlier.delta_since(later), AllocationDelta::default());
    }

    #[test]
    fn reporting_allocator_counts_all_system_allocation_operations() {
        // This test is the only allocator-wrapper user in this test binary.
        // Ordinary test-harness allocations use System directly, so they do
        // not enter these counters. Do not install a global wrapper here.
        let allocator = CountingAllocator::system();
        let initial = snapshot();
        let small = Layout::from_size_align(16, 8).unwrap();
        let large = Layout::from_size_align(32, 8).unwrap();
        // SAFETY: each successful allocation is released exactly once using
        // its current layout, and failed reallocations retain the old pointer.
        unsafe {
            let pointer = allocator.alloc(small);
            assert!(!pointer.is_null());
            let resized = allocator.realloc(pointer, small, large.size());
            if resized.is_null() {
                allocator.dealloc(pointer, small);
                panic!("system reallocation failed");
            }
            allocator.dealloc(resized, large);
            let zeroed = allocator.alloc_zeroed(small);
            assert!(!zeroed.is_null());
            assert!(
                std::slice::from_raw_parts(zeroed, small.size())
                    .iter()
                    .all(|byte| *byte == 0)
            );
            allocator.dealloc(zeroed, small);
        }
        assert_eq!(
            snapshot().delta_since(initial),
            AllocationDelta {
                allocation_calls: 3,
                deallocation_calls: 3,
                allocated_bytes: 64,
                deallocated_bytes: 64,
            }
        );
    }
}
