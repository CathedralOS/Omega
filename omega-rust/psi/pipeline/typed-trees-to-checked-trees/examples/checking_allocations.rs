//! Manual whole-checking measurement, without test-only flow replay or harness allocations.
//! Run: cargo run -p typed-trees-to-checked-trees --example checking_allocations
//! Counts successful allocation/reallocation requests and requested bytes, not peak RSS.

use std::alloc::{GlobalAlloc, Layout, System};
use std::fmt::Write;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

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

fn source(chain_length: usize, independent_machines: usize, contracts: bool) -> String {
    let mut source = String::from("machine chain() -> u8 { transition { _ -> step0(3) }\n");
    for state_index in (0..chain_length).rev() {
        write!(source, "state step{state_index}(value: u8) -> u8 {{ ").unwrap();
        if state_index + 1 == chain_length {
            source.push_str("value");
        } else {
            write!(
                source,
                "transition {{ _ -> step{}(value) }}",
                state_index + 1
            )
            .unwrap();
        }
        source.push_str(" }\n");
    }
    source.push_str("}\n");
    for machine_index in 0..independent_machines {
        if contracts {
            writeln!(source, "machine independent{machine_index}(input: u8) -> u8 requires input < 200; ensures result == 7 {{ 7 }}").unwrap();
        } else {
            writeln!(source, "machine independent{machine_index}() -> u8 {{ 7 }}").unwrap();
        }
    }
    source
}

fn main() {
    for (chain_length, independent_machines, contracts) in [
        (1, 0, false),
        (12, 64, false),
        (12, 64, true),
        (32, 256, true),
    ] {
        let source = source(chain_length, independent_machines, contracts);
        let tokens = source_files_to_tokens::Lexer::new(&source)
            .tokenize()
            .unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
        let program =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        let expected = typed_trees_to_checked_trees::lower_typed_trees(program.clone()).unwrap();
        let mut elapsed = Vec::new();
        let mut allocation_calls = Vec::new();
        let mut allocated_bytes = Vec::new();
        for _ in 0..7 {
            let input = program.clone();
            let before_calls = ALLOCATION_CALLS.load(Ordering::Relaxed);
            let before_bytes = ALLOCATED_BYTES.load(Ordering::Relaxed);
            let start = Instant::now();
            let checked = typed_trees_to_checked_trees::lower_typed_trees(input).unwrap();
            let duration = start.elapsed();
            let calls = ALLOCATION_CALLS.load(Ordering::Relaxed) - before_calls;
            let bytes = ALLOCATED_BYTES.load(Ordering::Relaxed) - before_bytes;
            assert_eq!(
                checked, expected,
                "repeated checking must retain identical evidence"
            );
            elapsed.push(duration);
            allocation_calls.push(calls);
            allocated_bytes.push(bytes);
        }
        elapsed.sort();
        allocation_calls.sort();
        allocated_bytes.sort();
        println!(
            "chain={chain_length} independent={independent_machines} contracts={contracts}: median={:?} allocations={} requested_bytes={}",
            elapsed[3], allocation_calls[3], allocated_bytes[3]
        );
    }
}
