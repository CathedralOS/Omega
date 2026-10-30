/*
   Opt-in Rust global-allocation accounting. Wrap the application's real
   allocator, so the observer changes neither allocation policy nor ownership.
   Counters describe successful requested bytes, not allocator capacity or RSS.
   The peak tracks process-lifetime requested storage across all Rust threads at
   wrapper bookkeeping points. It is not a physical allocator high-water mark;
   simultaneous reallocations can move storage before their updates are visible.
   Failed allocations are counted separately; failed realloc retains its old
   allocation. Snapshots are intended after workers join, not as atomic views of
   simultaneous allocator calls. Use this instrumented build for attribution,
   separately from uninstrumented timing comparisons.
*/

use std::alloc::{GlobalAlloc, Layout};
use std::sync::atomic::{AtomicU64, Ordering};

pub struct ObservedAllocator<Allocator> {
    allocator: Allocator,
    allocations: AtomicU64,
    reallocations: AtomicU64,
    deallocations: AtomicU64,
    requested_bytes: AtomicU64,
    live_bytes: AtomicU64,
    peak_live_bytes: AtomicU64,
    failures: AtomicU64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Snapshot {
    pub allocations: u64,
    pub reallocations: u64,
    pub deallocations: u64,
    pub requested_bytes: u64,
    pub live_bytes: u64,
    pub peak_live_bytes: u64,
    pub failures: u64,
}

impl<Allocator> ObservedAllocator<Allocator> {
    pub const fn new(allocator: Allocator) -> Self {
        Self {
            allocator,
            allocations: AtomicU64::new(0),
            reallocations: AtomicU64::new(0),
            deallocations: AtomicU64::new(0),
            requested_bytes: AtomicU64::new(0),
            live_bytes: AtomicU64::new(0),
            peak_live_bytes: AtomicU64::new(0),
            failures: AtomicU64::new(0),
        }
    }

    fn grow_live(&self, bytes: u64) {
        let live = self.live_bytes.fetch_add(bytes, Ordering::Relaxed) + bytes;
        self.peak_live_bytes.fetch_max(live, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            allocations: self.allocations.load(Ordering::Relaxed),
            reallocations: self.reallocations.load(Ordering::Relaxed),
            deallocations: self.deallocations.load(Ordering::Relaxed),
            requested_bytes: self.requested_bytes.load(Ordering::Relaxed),
            live_bytes: self.live_bytes.load(Ordering::Relaxed),
            peak_live_bytes: self.peak_live_bytes.load(Ordering::Relaxed),
            failures: self.failures.load(Ordering::Relaxed),
        }
    }

    pub fn write_report(&self, path: &std::path::Path) -> std::io::Result<()> {
        use std::io::Write;
        let snapshot = self.snapshot();
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        write!(
            file,
            "{{\"schema\":\"omega-allocation-observation/1\",\"scope\":\"process-lifetime Rust global allocator; instrumentation enabled\",\"allocations\":{},\"reallocations\":{},\"deallocations\":{},\"requested_bytes\":{},\"live_bytes\":{},\"peak_live_bytes\":{},\"failures\":{}}}",
            snapshot.allocations,
            snapshot.reallocations,
            snapshot.deallocations,
            snapshot.requested_bytes,
            snapshot.live_bytes,
            snapshot.peak_live_bytes,
            snapshot.failures
        )?;
        file.sync_data()
    }
}

// SAFETY: every operation delegates to the same allocator with the caller's
// original layout/pointer. Bookkeeping touches only independent atomics.
unsafe impl<Allocator: GlobalAlloc> GlobalAlloc for ObservedAllocator<Allocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: this is precisely GlobalAlloc::alloc's caller contract.
        let pointer = unsafe { self.allocator.alloc(layout) };
        if pointer.is_null() {
            self.failures.fetch_add(1, Ordering::Relaxed);
        } else {
            self.allocations.fetch_add(1, Ordering::Relaxed);
            self.requested_bytes
                .fetch_add(layout.size() as u64, Ordering::Relaxed);
            self.grow_live(layout.size() as u64);
        }
        pointer
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the delegated operation retains the zero-initialization contract.
        let pointer = unsafe { self.allocator.alloc_zeroed(layout) };
        if pointer.is_null() {
            self.failures.fetch_add(1, Ordering::Relaxed);
        } else {
            self.allocations.fetch_add(1, Ordering::Relaxed);
            self.requested_bytes
                .fetch_add(layout.size() as u64, Ordering::Relaxed);
            self.grow_live(layout.size() as u64);
        }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // Retire before releasing storage: another thread may immediately reuse
        // it while the underlying allocator's dealloc call is still returning.
        self.deallocations.fetch_add(1, Ordering::Relaxed);
        self.live_bytes
            .fetch_sub(layout.size() as u64, Ordering::Relaxed);
        // SAFETY: caller supplies the live pointer and matching original layout.
        unsafe { self.allocator.dealloc(pointer, layout) };
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: delegated pointer, old layout and new size retain caller's contract.
        let resized = unsafe { self.allocator.realloc(pointer, layout, new_size) };
        if resized.is_null() {
            self.failures.fetch_add(1, Ordering::Relaxed);
        } else {
            self.reallocations.fetch_add(1, Ordering::Relaxed);
            self.requested_bytes
                .fetch_add(new_size as u64, Ordering::Relaxed);
            if new_size >= layout.size() {
                self.grow_live((new_size - layout.size()) as u64);
            } else {
                self.live_bytes
                    .fetch_sub((layout.size() - new_size) as u64, Ordering::Relaxed);
            }
        }
        resized
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn freed_storage_is_retired_before_another_thread_reuses_it() {
        use std::sync::{Arc, Barrier, atomic::AtomicBool};
        struct PausingSystem {
            first_free: AtomicBool,
            freed: Arc<Barrier>,
            resume: Arc<Barrier>,
        }
        // SAFETY: storage and layouts are delegated unchanged to System.
        unsafe impl GlobalAlloc for PausingSystem {
            unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
                unsafe { std::alloc::System.alloc(layout) }
            }
            unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
                unsafe { std::alloc::System.dealloc(pointer, layout) };
                if !self.first_free.swap(true, Ordering::Relaxed) {
                    self.freed.wait();
                    self.resume.wait();
                }
            }
        }
        let freed = Arc::new(Barrier::new(2));
        let resume = Arc::new(Barrier::new(2));
        let allocator = Arc::new(ObservedAllocator::new(PausingSystem {
            first_free: AtomicBool::new(false),
            freed: freed.clone(),
            resume: resume.clone(),
        }));
        let small = Layout::from_size_align(32, 8).unwrap();
        let large = Layout::from_size_align(64, 8).unwrap();
        // SAFETY: ownership of the first pointer transfers exclusively to the
        // freeing thread. The second pointer is freed on this thread.
        let pointer = unsafe { allocator.alloc(small) };
        assert!(!pointer.is_null());
        let address = pointer as usize;
        let worker_allocator = allocator.clone();
        let worker = std::thread::spawn(move || unsafe {
            worker_allocator.dealloc(address as *mut u8, small);
        });
        freed.wait();
        let reused = unsafe { allocator.alloc(large) };
        assert!(!reused.is_null());
        let live = allocator.snapshot().live_bytes;
        unsafe { allocator.dealloc(reused, large) };
        resume.wait();
        worker.join().unwrap();
        assert_eq!(live, 64);
        assert_eq!(allocator.snapshot().peak_live_bytes, 64);
        assert_eq!(allocator.snapshot().live_bytes, 0);
    }
    #[test]
    fn recovers_known_live_peak_and_reallocation_counts() {
        let allocator = ObservedAllocator::new(std::alloc::System);
        let original = Layout::from_size_align(64, 8).unwrap();
        // SAFETY: each allocated region is resized and freed with its exact layout.
        unsafe {
            let pointer = allocator.alloc(original);
            assert!(!pointer.is_null());
            let resized = allocator.realloc(pointer, original, 128);
            assert!(!resized.is_null());
            assert_eq!(allocator.snapshot().live_bytes, 128);
            allocator.dealloc(resized, Layout::from_size_align(128, 8).unwrap());
        }
        let observed = allocator.snapshot();
        assert_eq!(
            (
                observed.allocations,
                observed.reallocations,
                observed.deallocations
            ),
            (1, 1, 1)
        );
        assert_eq!(
            (
                observed.requested_bytes,
                observed.live_bytes,
                observed.peak_live_bytes
            ),
            (192, 0, 128)
        );
    }
    #[test]
    fn failed_reallocation_does_not_retire_the_original_storage() {
        struct Refusing;
        // SAFETY: this allocator delegates live allocation ownership to System.
        unsafe impl GlobalAlloc for Refusing {
            unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
                unsafe { std::alloc::System.alloc(layout) }
            }
            unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
                unsafe { std::alloc::System.dealloc(pointer, layout) }
            }
            unsafe fn realloc(&self, _: *mut u8, _: Layout, _: usize) -> *mut u8 {
                std::ptr::null_mut()
            }
        }
        let allocator = ObservedAllocator::new(Refusing);
        let layout = Layout::from_size_align(32, 8).unwrap();
        unsafe {
            let pointer = allocator.alloc_zeroed(layout);
            assert!(!pointer.is_null());
            assert!(allocator.realloc(pointer, layout, 64).is_null());
            assert_eq!(allocator.snapshot().live_bytes, 32);
            allocator.dealloc(pointer, layout);
        }
        assert_eq!(allocator.snapshot().failures, 1);
        assert_eq!(allocator.snapshot().live_bytes, 0);
    }
}
