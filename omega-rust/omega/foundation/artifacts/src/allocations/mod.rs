//! Process-wide allocation instrumentation for compiler phase reports.
//! Explicit measurement hosts may install the wrapper; this owner supplies both
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
//! wrapper. The normal `omega` binary does not install it. Under a
//! test binary, a bench harness, or any other host that does not use the wrapper,
//! every snapshot reads zero
//! and the phase report shows zero bytes allocated — which reads as a
//! measurement rather than as an absent one. There is nothing in the API that
//! lets a caller tell the two apart.

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
}

impl AllocationDelta {
}

pub fn snapshot() -> AllocationSnapshot {
    AllocationSnapshot {
        allocation_calls: ALLOCATION_CALLS.load(Ordering::Relaxed),
        deallocation_calls: DEALLOCATION_CALLS.load(Ordering::Relaxed),
        allocated_bytes: ALLOCATED_BYTES.load(Ordering::Relaxed),
        deallocated_bytes: DEALLOCATED_BYTES.load(Ordering::Relaxed),
    }
}




