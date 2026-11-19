//! Count allocations on the thread executing one real retained-program clone.

use std::alloc::{GlobalAlloc, Layout};
use std::cell::Cell;
use std::io::Write;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

#[derive(Clone, Copy, Debug, Default)]
pub struct Counts {
    pub allocations: usize,
    pub requested_bytes: usize,
    pub live_delta: isize,
    pub peak_delta: isize,
    pub nanoseconds: u128,
    pub arena_clones: usize,
    pub arena_slots: usize,
    pub arena_payload_bytes: usize,
}

thread_local! {
    static COUNTS: Cell<Option<Counts>> = const { Cell::new(None) };
    static CONTEXT: Cell<&'static str> = const { Cell::new("other") };
}

pub fn directory() -> Option<&'static std::path::Path> {
    static DIRECTORY: OnceLock<Option<std::path::PathBuf>> = OnceLock::new();
    DIRECTORY
        .get_or_init(|| {
            std::env::var_os("OMEGA_SELECTED_BODY_PROFILE").map(|value| {
                let directory = std::path::PathBuf::from(value);
                std::fs::create_dir(&directory).expect("new profile directory can be created");
                directory
            })
        })
        .as_deref()
}

pub fn log(arguments: std::fmt::Arguments<'_>) {
    let Some(directory) = directory() else {
        return;
    };
    static LOCK: Mutex<()> = Mutex::new(());
    let _guard = LOCK.lock().expect("profile log lock is not poisoned");
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(directory.join("profile.log"))
        .expect("profile log opens");
    writeln!(file, "{arguments}").expect("profile row writes");
}

pub struct Context(&'static str);

pub fn context(label: &'static str) -> Context {
    Context(CONTEXT.replace(label))
}

impl Drop for Context {
    fn drop(&mut self) {
        CONTEXT.set(self.0);
    }
}

pub fn current_context() -> &'static str {
    CONTEXT.get()
}

pub fn measure<T>(operation: impl FnOnce() -> T) -> (T, Counts) {
    assert!(COUNTS.replace(Some(Counts::default())).is_none());
    // The window ends before logging/exporting or examining semantic equality.
    let start = Instant::now();
    let value = operation();
    let elapsed = start.elapsed().as_nanos();
    let mut counts = COUNTS.replace(None).expect("active allocation window");
    counts.nanoseconds = elapsed;
    (value, counts)
}

pub fn record_arena_clone(slots: usize, slot_bytes: usize) {
    let _ = COUNTS.try_with(|slot| {
        if let Some(mut counts) = slot.get() {
            counts.arena_clones += 1;
            counts.arena_slots += slots;
            counts.arena_payload_bytes += slots.saturating_mul(slot_bytes);
            slot.set(Some(counts));
        }
    });
}

fn allocated(size: usize) {
    let _ = COUNTS.try_with(|slot| {
        if let Some(mut counts) = slot.get() {
            counts.allocations += 1;
            counts.requested_bytes += size;
            counts.live_delta += size as isize;
            counts.peak_delta = counts.peak_delta.max(counts.live_delta);
            slot.set(Some(counts));
        }
    });
}

fn deallocated(size: usize) {
    let _ = COUNTS.try_with(|slot| {
        if let Some(mut counts) = slot.get() {
            counts.live_delta -= size as isize;
            slot.set(Some(counts));
        }
    });
}

pub struct Tracking<A>(pub A);

// SAFETY: Requests and returned pointers pass unchanged to the wrapped
// allocator. The observer retains numbers only and never accesses payloads.
unsafe impl<A: GlobalAlloc> GlobalAlloc for Tracking<A> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { self.0.alloc(layout) };
        if !pointer.is_null() {
            allocated(layout.size());
        }
        pointer
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { self.0.alloc_zeroed(layout) };
        if !pointer.is_null() {
            allocated(layout.size());
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        deallocated(layout.size());
        unsafe { self.0.dealloc(pointer, layout) };
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let result = unsafe { self.0.realloc(pointer, layout, size) };
        if !result.is_null() {
            deallocated(layout.size());
            allocated(size);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[global_allocator]
    static ALLOCATOR: Tracking<std::alloc::System> = Tracking(std::alloc::System);

    #[test]
    fn windows_exclude_setup_and_remain_thread_local() {
        let _outside = std::hint::black_box(Vec::<u8>::with_capacity(8192));
        let threads = [257, 509].map(|bytes| {
            std::thread::spawn(move || {
                let (payload, counts) = measure(|| {
                    std::hint::black_box(Vec::<u8>::with_capacity(std::hint::black_box(bytes)))
                });
                assert_eq!(payload.capacity(), bytes);
                assert_eq!(counts.allocations, 1);
                assert_eq!(counts.requested_bytes, bytes);
                assert_eq!(counts.live_delta, bytes as isize);
                assert_eq!(counts.peak_delta, bytes as isize);
                assert!(COUNTS.get().is_none());
            })
        });
        for thread in threads {
            thread.join().unwrap();
        }
        assert!(COUNTS.get().is_none());
    }
}
