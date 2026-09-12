//! Measures allocation requests during complete static-machine specialization.
//! Run: cargo run -p typed-trees-to-checked-trees --example specialization_allocations
//! Input construction and cloning are excluded. Requested bytes are not peak RSS.

use std::alloc::{GlobalAlloc, Layout, System};
use std::fmt::Write;
use std::sync::atomic::{AtomicUsize, Ordering};

struct CountAllocations;

static ALLOCATION_CALLS: AtomicUsize = AtomicUsize::new(0);
static ALLOCATED_BYTES: AtomicUsize = AtomicUsize::new(0);
#[global_allocator]
static ALLOCATOR: CountAllocations = CountAllocations;

fn record(pointer: *mut u8, bytes: usize) -> *mut u8 {
    if !pointer.is_null() {
        ALLOCATION_CALLS.fetch_add(1, Ordering::Relaxed);
        ALLOCATED_BYTES.fetch_add(bytes, Ordering::Relaxed);
    }
    pointer
}

// SAFETY: Requests and returned pointers are forwarded unchanged to System;
// counters do not allocate and no pointer is retained or dereferenced here.
unsafe impl GlobalAlloc for CountAllocations {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record(unsafe { System.alloc(layout) }, layout.size())
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record(unsafe { System.alloc_zeroed(layout) }, layout.size())
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        record(unsafe { System.realloc(pointer, layout, size) }, size)
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
}

fn program(template_count: usize, independent_machines: usize) -> typed_trees::TypedTrees {
    let mut source = String::new();
    for ordinal in 0..template_count {
        writeln!(
            source,
            "machine value{ordinal}<const N: u64>() -> u64 {{ N }}"
        )
        .unwrap();
        writeln!(
            source,
            "machine first{ordinal}() -> u64 {{ value{ordinal}<7>() }}"
        )
        .unwrap();
        writeln!(
            source,
            "machine second{ordinal}() -> u64 {{ value{ordinal}<9>() }}"
        )
        .unwrap();
        writeln!(
            source,
            "machine third{ordinal}() -> u64 {{ value{ordinal}<11>() }}"
        )
        .unwrap();
    }
    for ordinal in 0..independent_machines {
        writeln!(source, "machine unrelated{ordinal}() -> u64 {{").unwrap();
        for local in 0..16 {
            writeln!(source, "let item{local}: u64 = {local};").unwrap();
        }
        source.push_str("7 }\n");
    }
    let tokens = source_files_to_tokens::Lexer::new(&source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
}

fn main() {
    for (templates, unrelated) in [(1, 0), (1, 64), (8, 0), (8, 64)] {
        let input = program(templates, unrelated);
        let mut expected = input.clone();
        typed_trees_to_checked_trees::specialize_static_machine_calls(&mut expected).unwrap();
        assert_eq!(expected.machine_specializations.len(), templates * 3);
        let mut measurements = Vec::new();
        for _ in 0..3 {
            let mut current = input.clone();
            let calls_before = ALLOCATION_CALLS.load(Ordering::Relaxed);
            let bytes_before = ALLOCATED_BYTES.load(Ordering::Relaxed);
            typed_trees_to_checked_trees::specialize_static_machine_calls(&mut current).unwrap();
            let calls = ALLOCATION_CALLS.load(Ordering::Relaxed) - calls_before;
            let bytes = ALLOCATED_BYTES.load(Ordering::Relaxed) - bytes_before;
            assert_eq!(current, expected, "specialization output is deterministic");
            measurements.push((calls, bytes));
        }
        assert!(
            measurements
                .iter()
                .all(|measurement| *measurement == measurements[0])
        );
        println!(
            "templates={templates} unrelated={unrelated} allocations={} requested_bytes={}",
            measurements[0].0, measurements[0].1
        );
    }
}
