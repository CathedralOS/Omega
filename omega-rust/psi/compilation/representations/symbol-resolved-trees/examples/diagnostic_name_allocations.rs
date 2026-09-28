//! Run with `mbx run -p symbol-resolved-trees --example diagnostic_name_allocations`.
//! A single-thread process excludes allocations by the Rust test harness.

use source::SourceSpan;
use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use symbol_resolved_trees::name::DiagnosticName;

struct CountAllocations;
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
#[global_allocator]
static ALLOCATOR: CountAllocations = CountAllocations;
// SAFETY: All allocation requests are forwarded unchanged to the system allocator.
unsafe impl GlobalAlloc for CountAllocations {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.realloc(pointer, layout, size) }
    }
}
fn main() {
    let spelling = black_box("diagnostic::naïve::spelling");
    ALLOCATIONS.store(0, Ordering::Relaxed);
    let previous: Arc<str> = Arc::from(String::from(spelling).into_boxed_str());
    black_box(&previous);
    let previous_count = ALLOCATIONS.load(Ordering::Relaxed);
    ALLOCATIONS.store(0, Ordering::Relaxed);
    let current = DiagnosticName::from_str(spelling, SourceSpan::default());
    black_box(&current);
    let current_count = ALLOCATIONS.load(Ordering::Relaxed);
    ALLOCATIONS.store(0, Ordering::Relaxed);
    let converted = DiagnosticName::from(spelling);
    black_box(&converted);
    let converted_count = ALLOCATIONS.load(Ordering::Relaxed);
    assert_eq!(previous_count, 2);
    assert_eq!(current_count, 1);
    assert_eq!(converted_count, 1);
    assert_eq!(&*previous, current.as_str());
    println!(
        "Previous borrowed construction: {previous_count} allocations; direct borrowed construction: {current_count}; From<&str>: {converted_count}"
    );
}
