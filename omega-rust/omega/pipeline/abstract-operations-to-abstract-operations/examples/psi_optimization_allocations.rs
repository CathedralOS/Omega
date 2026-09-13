//! Manual multi-rule optimizer-run measurement: allocation and wall-time cost of
//! whole `run_psi_pipeline` executions, attributed against the per-revision
//! canonical identity recompute (`recompute_psi_optimization_unit_identity`).
//! Run: cargo run -p abstract-operations-to-abstract-operations --example psi_optimization_allocations
//! Counts successful allocation/reallocation requests and requested bytes, not peak RSS.

use std::alloc::{GlobalAlloc, Layout, System};
use std::fmt::Write;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use abstract_operations_to_abstract_operations::run_psi_pipeline;
use optimization_core::{Optimization, OptimizationSelections, OptimizationWorkBudget};
use optimization_unit::recompute_psi_optimization_unit_identity;
use terminal_psi_to_abstract_operations::{
    VerifiedPsiOptimizationUnit, build_verified_psi_optimization_unit,
    lower_artifact_sections_for_optimization,
};

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

/// Straight-line wrapping arithmetic over parameters: every node is live and
/// non-constant, so the full selected suite evaluates and applies nothing.
fn unchanged_source(operations: usize) -> String {
    let mut expression = String::from("input");
    for index in 0..operations {
        let operand = match index % 3 {
            0 => "other".to_owned(),
            _ => format!("{}u64", 3 + 2 * index),
        };
        let operator = ["+", "*", "-"][index % 3];
        write!(expression, " {operator} {operand}").unwrap();
        expression = format!("({expression})");
    }
    format!(
        "machine measure(input: u64 in Wrapping, other: u64 in Wrapping) -> u64 {{ ({expression}) as u64 }}\n"
    )
}

/// Straight-line exact arithmetic over literals: constant-folding rules commit
/// repeatedly, so every revision re-encodes changed content.
fn fold_source(operations: usize) -> String {
    let mut expression = String::from("1u64");
    for index in 0..operations {
        let operator = ["+", "*", "+"][index % 3];
        expression = format!("({expression} {operator} {}u64)", 3 + index);
    }
    format!("machine measure() -> u64 {{ {expression} }}\n")
}

fn verified_unit(source: &str) -> VerifiedPsiOptimizationUnit {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved =
        syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolve");
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed).expect("check");
    let lowered =
        checked_trees_to_lowered_psi::lower_machine(&checked, "measure").expect("lower measure");
    let semantic =
        terminal_codec::encode_module(&lowered.semantic_module).expect("encode semantics");
    let proof = terminal_codec::encode_proof_bundle(&lowered.proof_bundle).expect("encode proof");
    let input = lower_artifact_sections_for_optimization(
        &semantic,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("verify for optimizer admission");
    build_verified_psi_optimization_unit(
        input,
        terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .expect("build optimization unit")
}

fn recompute_cost(unit: &optimization_unit::PsiOptimizationUnit) -> (u128, usize, usize) {
    let mut elapsed = Vec::new();
    let mut calls = Vec::new();
    let mut bytes = Vec::new();
    for _ in 0..7 {
        let before_calls = ALLOCATION_CALLS.load(Ordering::Relaxed);
        let before_bytes = ALLOCATED_BYTES.load(Ordering::Relaxed);
        let start = Instant::now();
        let mut identity = recompute_psi_optimization_unit_identity(unit);
        for _ in 0..8 {
            identity = recompute_psi_optimization_unit_identity(unit);
        }
        let duration = start.elapsed();
        let run_calls = ALLOCATION_CALLS.load(Ordering::Relaxed) - before_calls;
        let run_bytes = ALLOCATED_BYTES.load(Ordering::Relaxed) - before_bytes;
        assert_eq!(identity, unit.identity, "recompute replays stored identity");
        elapsed.push(duration.as_micros() / 9);
        calls.push(run_calls / 9);
        bytes.push(run_bytes / 9);
    }
    elapsed.sort();
    calls.sort();
    bytes.sort();
    (elapsed[3], calls[3], bytes[3])
}

fn measure(name: &str, source: String, selections: &OptimizationSelections) {
    let verified = verified_unit(&source);
    let unit = verified.unit();
    let nodes: usize = unit
        .functions
        .iter()
        .flat_map(|function| function.blocks.iter())
        .map(|block| block.nodes.len())
        .sum();
    let (identity_micros, identity_calls, identity_bytes) = recompute_cost(unit);
    let budget = OptimizationWorkBudget::new(1_000_000, 1_000_000, 1_000_000, 64_000, 64_000)
        .expect("measurement budget");
    let mut elapsed = Vec::new();
    let mut allocation_calls = Vec::new();
    let mut allocated_bytes = Vec::new();
    let mut usage = None;
    let mut commits = 0;
    for rep in 0..5 {
        let input = verified.clone();
        let before_calls = ALLOCATION_CALLS.load(Ordering::Relaxed);
        let before_bytes = ALLOCATED_BYTES.load(Ordering::Relaxed);
        let start = Instant::now();
        let run = run_psi_pipeline(input, selections, budget).expect("optimizer run");
        let duration = start.elapsed();
        let calls = ALLOCATION_CALLS.load(Ordering::Relaxed) - before_calls;
        let bytes = ALLOCATED_BYTES.load(Ordering::Relaxed) - before_bytes;
        eprintln!("  {name} rep{rep}: {duration:?} {calls}allocs {bytes}B");
        elapsed.push(duration);
        allocation_calls.push(calls);
        allocated_bytes.push(bytes);
        commits = run.commits.len();
        usage = Some(run.usage);
    }
    elapsed.sort();
    allocation_calls.sort();
    allocated_bytes.sort();
    let usage = usage.expect("one measured run");
    // Content-identity recompute happens once per bound revision (one per
    // pass iteration), once per committed rewrite, and once per validated
    // candidate's produced unit — `AnalysisRevision` requests reuse the
    // already-validated immutable borrow and do not re-encode.
    let identity_recomputes = usage.iterations + usage.commits + usage.validation_steps;
    let identity_total_bytes = identity_recomputes as usize * identity_bytes;
    println!(
        "{name}: nodes={nodes} median={:?} allocations={} requested_bytes={}",
        elapsed[2], allocation_calls[2], allocated_bytes[2]
    );
    println!(
        "  iterations={} rule_evals={} candidates={} validations={} commits={commits}",
        usage.iterations, usage.rule_evaluations, usage.candidates, usage.validation_steps
    );
    println!(
        "  identity: recomputes={identity_recomputes} per_call={identity_micros}us {identity_calls}allocs {identity_bytes}B => ~{identity_total_bytes}B ({:.1}% of run bytes)",
        100.0 * identity_total_bytes as f64 / allocated_bytes[2].max(1) as f64
    );
}

fn main() {
    // Deep generated expressions recurse through the frontend; the compiler
    // makes the same provision in `omega/src/main.rs`.
    std::thread::Builder::new()
        .stack_size(256 * 1024 * 1024)
        .spawn(measure_scenarios)
        .expect("measurement worker")
        .join()
        .expect("measurement worker result");
}

fn measure_scenarios() {
    let selections = OptimizationSelections::new([
        Optimization::ControlFlowCleanup,
        Optimization::SparseConditionalConstantPropagation,
        Optimization::CopyPropagation,
        Optimization::GlobalValueNumbering,
        Optimization::DeadPureScalarElimination,
        Optimization::ProofCheckElision,
    ])
    .expect("multi-rule selection");
    // PSI_ALLOC_SCENARIOS: comma-separated substrings matched against each
    // scenario name; unset runs everything.
    let filter = std::env::var("PSI_ALLOC_SCENARIOS")
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let selected = |name: &str, source: String| {
        if filter.is_empty() || filter.iter().any(|item| name.contains(item)) {
            measure(name, source, &selections);
        }
    };
    for operations in [128, 512] {
        selected(
            &format!("unchanged ops={operations}"),
            unchanged_source(operations),
        );
    }
    for operations in [32, 64] {
        selected(
            &format!("fold      ops={operations}"),
            fold_source(operations),
        );
    }
}
