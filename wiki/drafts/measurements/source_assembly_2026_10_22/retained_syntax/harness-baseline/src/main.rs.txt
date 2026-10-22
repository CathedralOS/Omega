use std::{
    alloc::{GlobalAlloc, Layout},
    cell::Cell,
    hint::black_box,
    path::PathBuf,
    sync::Arc,
};

thread_local! {static COUNTS:Cell<(bool,u64,u64)> = const { Cell::new((false,0,0)) };}
struct Allocator;
fn count(bytes: usize) {
    COUNTS.with(|state| {
        let (enabled, calls, total) = state.get();
        if enabled {
            state.set((true, calls + 1, total + bytes as u64));
        }
    });
}
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count(layout.size());
        unsafe { mimalloc::MiMalloc.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count(layout.size());
        unsafe { mimalloc::MiMalloc.alloc_zeroed(layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { mimalloc::MiMalloc.dealloc(pointer, layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        count(size);
        unsafe { mimalloc::MiMalloc.realloc(pointer, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;

use artifacts::compile_timings::CompileTimings;
use package_compilation::source_assembly::ImmutableSourceParseCheckpoint;
use sha2::{Digest, Sha256};
fn main() {
    std::thread::Builder::new()
        .stack_size(256 * 1024 * 1024)
        .spawn(record)
        .unwrap()
        .join()
        .unwrap();
}
fn record() {
    let paths = std::fs::read_to_string(std::env::args().nth(1).unwrap()).unwrap();
    for path in paths.lines() {
        let path = PathBuf::from(path).canonicalize().unwrap();
        let mut timings = CompileTimings::default();
        COUNTS.with(|state| state.set((true, 0, 0)));
        let start = std::time::Instant::now();
        let result =
            ImmutableSourceParseCheckpoint::prepare(black_box(&path), None, false, &mut timings)
                .and_then(|checkpoint| {
                    let retained = std::env::args()
                        .any(|value| value == "--shared")
                        .then(|| checkpoint.clone());
                    let result = checkpoint.assemble(None, &mut timings);
                    black_box(&retained);
                    result
                });
        black_box(&result);
        let nanos = start.elapsed().as_nanos();
        let counts = COUNTS.with(|state| {
            let result = state.get();
            state.set((false, 0, 0));
            result
        });
        let mut root_metrics = (0, 0, 0, 0, 0);
        if let Ok((_, syntax)) = &result {
            let loaded = syntax
                .sources
                .files()
                .map(|file| source::LoadedSource {
                    source_id: file.source_id,
                    path: file.path.clone(),
                    source: file.source.clone(),
                    origin: Some(file.origin),
                    scope: file.dependency_scope,
                })
                .collect();
            let mut trees = syntax_trees::SyntaxTrees::default();
            let parsed = psi_compiler::SourceInput::Retained(loaded)
                .append(&mut trees, &mut psi_compiler::UnmeasuredProduction)
                .unwrap();
            for file in parsed.sources.span_or_empty(parsed.batch) {
                root_metrics.3 += 1;
                root_metrics.4 += file.path.as_os_str().len();
                root_metrics.0 += usize::from(!file.root_items.is_empty());
                root_metrics.1 +=
                    file.root_items.len() * std::mem::size_of::<syntax_trees::item::ItemHandle>();
                root_metrics.2 += file.root_items.capacity()
                    * std::mem::size_of::<syntax_trees::item::ItemHandle>();
            }
        }
        let (status, output) = match result {
            Ok((count, syntax)) => (
                "assembled",
                format!(
                    "{count}\n{:?}\n{:?}\n{:?}\n{:?}\n{:?}",
                    syntax.syntax_trees,
                    syntax.sources,
                    syntax.build_source_id,
                    syntax.application,
                    (
                        syntax.source_scoped_top_level_bindings,
                        syntax.generated_source_custody,
                        {
                            let mut sources =
                                syntax.build_scope_sources.into_iter().collect::<Vec<_>>();
                            sources.sort_by_key(|source| source.0);
                            sources
                        }
                    )
                ),
            ),
            Err(errors) => ("rejected", format!("{errors:?}")),
        };
        println!(
            "{}\t{}\t{}\t{}\t{:x}\t{}",
            counts.1,
            counts.2,
            nanos,
            status,
            Sha256::digest(output.as_bytes()),
            path.display()
        );
        eprintln!(
            "ROOTS\t{}\t{}\t{}\t{}\t{}\t{}",
            root_metrics.0,
            root_metrics.1,
            root_metrics.2,
            root_metrics.3,
            root_metrics.4,
            path.display()
        );
    }
}
