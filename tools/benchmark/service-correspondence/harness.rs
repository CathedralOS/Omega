// Both drivers call the production validator on ordinary parsed declarations.
// Parsing, projection and interner warm-up are outside the measured interval.
use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;
use typed_trees::provider_declarations::ProviderPlanningContext;
use typed_trees::service_declarations::projection;

struct MeasuredAllocator;
#[global_allocator]
static ALLOCATOR: MeasuredAllocator = MeasuredAllocator;
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static REQUESTED: AtomicUsize = AtomicUsize::new(0);
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

fn allocated(bytes: usize) {
    ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
    REQUESTED.fetch_add(bytes, Ordering::Relaxed);
    let live = LIVE.fetch_add(bytes, Ordering::Relaxed) + bytes;
    PEAK.fetch_max(live, Ordering::Relaxed);
}

// SAFETY: System receives every pointer, layout and requested size unchanged.
// Counters retain only sizes and never read or change allocation contents.
unsafe impl GlobalAlloc for MeasuredAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if COUNT_ALLOCATIONS && !pointer.is_null() {
            allocated(layout.size());
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        if COUNT_ALLOCATIONS {
            LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        }
        unsafe { System.dealloc(pointer, layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let result = unsafe { System.realloc(pointer, layout, size) };
        if COUNT_ALLOCATIONS && !result.is_null() {
            LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
            allocated(size);
        }
        result
    }
}

fn typed(text: &str) -> typed_trees::TypedTrees {
    let mut sources = source::SourceMap::default();
    let source = sources
        .add_checked_instance(
            "service/main.omg".into(),
            text,
            "service".into(),
            Some(semantic_vocabulary::PackageKeyIdentity::from_digest([7; 32]).unwrap()),
            source::SourceOrigin::User,
            source::SourceResolutionStratum::Base,
            source::DependencyScope::Product,
        )
        .source_id;
    let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees_with_id(source, &tokens).unwrap();
    let mut request = syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax);
    request.sources = Some(Arc::new(sources));
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(request).unwrap();
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
}

fn projected<'program>(
    program: &'program typed_trees::TypedTrees,
    name: &str,
) -> projection::ProjectedServiceSurface<'program> {
    let declaration = program
        .traits()
        .iter()
        .find(|row| row.name.as_str() == name)
        .unwrap();
    projection::from_typed(
        ProviderPlanningContext::new(program, source::DependencyScope::Product),
        declaration,
    )
    .unwrap()
}

fn family(methods: usize, inherited: bool) -> String {
    let parent_methods = if inherited { methods / 2 } else { 0 };
    let mut text = String::new();
    if inherited {
        text.push_str("boundary trait Parent {\n");
        for method in 0..parent_methods {
            text.push_str(&format!(
                "machine operation{method}(value: u64) -> u64 reaches none crashes none;\n"
            ));
        }
        text.push_str("}\nboundary trait Service: Parent {\n");
    } else {
        text.push_str("boundary trait Service {\n");
    }
    for method in parent_methods..methods {
        text.push_str(&format!(
            "machine operation{method}(value: u64) -> u64 reaches none crashes none;\n"
        ));
    }
    text.push_str("}\n");
    text
}

fn workloads() -> Vec<(String, String, &'static str)> {
    let console = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../source/library/std/console.omg"
    ));
    let console = &console
        [console.find("public data ByteRead").unwrap()..console.find("// Text adapters").unwrap()];
    let mut rows = vec![
        ("one".into(), family(1, false), "Service"),
        ("target_selected".into(), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../../../tests/fixtures/run/providers/target_selected_implementation/main.omg")).into(), "TargetConstants"),
        ("console_declarations".into(), console.into(), "Console"),
    ];
    for methods in [16, 64] {
        rows.push((
            format!("inherited_{methods}"),
            family(methods, true),
            "Service",
        ));
    }
    rows
}

fn main() {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let option = |name| {
        arguments
            .windows(2)
            .find(|pair| pair[0] == name)
            .map(|pair| pair[1].as_str())
    };
    let samples = option("--samples").map_or(9, |value| value.parse::<usize>().unwrap());
    let iterations = option("--iterations").map(|value| value.parse::<usize>().unwrap());
    println!(
        "workload,methods,sample,iterations,ns_per_validation,allocations_per_validation,requested_bytes_per_validation,peak_extra_live_bytes"
    );
    for (label, text, name) in workloads() {
        if option("--only").is_some_and(|selected| selected != label) {
            continue;
        }
        let program = typed(&text);
        let projection = projected(&program, name);
        projection.validate_correspondence().unwrap();
        let methods = projection.declaration.methods.len();
        let iterations = iterations.unwrap_or((20_000 / methods.max(1).pow(2)).max(10));
        for sample in 0..samples {
            let allocations = ALLOCATIONS.load(Ordering::Relaxed);
            let requested = REQUESTED.load(Ordering::Relaxed);
            let live = LIVE.load(Ordering::Relaxed);
            PEAK.store(live, Ordering::Relaxed);
            let started = Instant::now();
            for _ in 0..iterations {
                black_box(black_box(&projection).validate_correspondence()).unwrap();
            }
            let elapsed = started.elapsed().as_nanos() as f64 / iterations as f64;
            let allocations =
                (ALLOCATIONS.load(Ordering::Relaxed) - allocations) as f64 / iterations as f64;
            let requested =
                (REQUESTED.load(Ordering::Relaxed) - requested) as f64 / iterations as f64;
            let peak = PEAK.load(Ordering::Relaxed).saturating_sub(live);
            println!(
                "{label},{methods},{sample},{iterations},{elapsed:.1},{allocations:.1},{requested:.1},{peak}"
            );
        }
    }
}
