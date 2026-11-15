//! Compare actual declaration-owner APIs from two explicitly built revisions.
//! The generated tables are metadata payloads, not executable Omega programs.

use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;
use symbols::{SymbolHandle, SymbolKind, SymbolNameRef, SymbolTableBuilder};
use typed_trees::machine::Machine;
use typed_trees::signature::StateParameter;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TableLocalData};
use typed_trees::types::TypeReferenceNode;
use typed_trees::{FrozenTypedTrees, TypedTrees};

struct MeasuredAllocator;
static COUNTING: AtomicBool = AtomicBool::new(false);
static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);

// Counters are enabled only for allocation audits, outside all timed runs.
unsafe impl GlobalAlloc for MeasuredAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNTING.load(Ordering::Relaxed) {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        }
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        if COUNTING.load(Ordering::Relaxed) {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            BYTES.fetch_add(size as u64, Ordering::Relaxed);
        }
        unsafe { System.realloc(pointer, layout, size) }
    }
}

#[global_allocator]
static ALLOCATOR: MeasuredAllocator = MeasuredAllocator;

#[derive(Clone, Copy)]
struct Shape {
    tiny: usize,
    large: usize,
    large_rows: usize,
    tiny_locals: usize,
    reverse: bool,
}

struct Payload {
    program: TypedTrees,
    local_symbols: Vec<SymbolHandle>,
    large_targets: Vec<(State, SymbolHandle)>,
}

fn payload(shape: Shape) -> Payload {
    let mut symbols = SymbolTableBuilder::default();
    let machine = symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static("owner"));
    let names: Vec<_> = (0..shape.tiny + shape.large)
        .map(|body| format!("body_{body}"))
        .collect();
    let state_symbols: Vec<_> = symbols
        .insert_children(
            machine,
            names
                .iter()
                .map(|name| (SymbolKind::State, SymbolNameRef::Borrowed(name))),
        )
        .handles()
        .collect();
    let mut body_symbols = Vec::new();
    for (body, state) in state_symbols.into_iter().enumerate() {
        let locals = if body < shape.tiny {
            shape.tiny_locals
        } else {
            shape.large_rows - 1
        };
        let local_names: Vec<_> = (0..locals).map(|local| format!("value_{local}")).collect();
        let mut binders = symbols
            .insert_children(
                state,
                std::iter::once((SymbolKind::Parameter, SymbolNameRef::Static("input"))).chain(
                    local_names
                        .iter()
                        .map(|name| (SymbolKind::Local, SymbolNameRef::Borrowed(name))),
                ),
            )
            .handles();
        let parameter = binders.next().unwrap();
        let locals: Vec<SymbolHandle> = binders.collect();
        body_symbols.push((state, parameter, locals));
    }
    let mut program = TypedTrees {
        symbols: symbols.finish(),
        ..Default::default()
    };
    let reference = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let mut states = Vec::new();
    let mut local_symbols = Vec::new();
    let mut large_targets = Vec::new();
    for (body, (symbol, parameter, locals)) in body_symbols.into_iter().enumerate() {
        let parameters = program.state_parameters.insert_many([StateParameter {
            symbol: parameter,
            type_reference: reference,
            ..Default::default()
        }]);
        let statement_nodes =
            program
                .statement_table
                .insert_statements(locals.iter().map(|&symbol| {
                    StatementNode::LocalData(TableLocalData {
                        symbol,
                        type_reference: reference,
                        ..Default::default()
                    })
                }));
        let state = State {
            symbol,
            name: typed_trees::name::Identifier::generated(names[body].clone()),
            parameters,
            statement_nodes,
            return_type: reference,
            ..Default::default()
        };
        local_symbols.extend_from_slice(&locals);
        if body >= shape.tiny {
            large_targets.push((state.clone(), *locals.last().unwrap()));
        }
        states.push(state);
    }
    if shape.reverse {
        states.reverse();
        large_targets.reverse();
    }
    let states = program.machine_states.insert_many(states);
    program.push_machine(Machine {
        symbol: machine,
        name: typed_trees::name::Identifier::generated("owner"),
        states,
        ..Default::default()
    });
    assert!(!local_symbols.is_empty() && !large_targets.is_empty());
    Payload {
        program,
        local_symbols,
        large_targets,
    }
}

fn global(program: &TypedTrees, payload: &Payload, query: usize) -> u64 {
    let symbol = payload.local_symbols[query % payload.local_symbols.len()];
    let local = program
        .unique_local_data_for_symbol(black_box(symbol))
        .expect("one exact published local");
    assert_eq!(local.symbol, symbol);
    black_box(local.type_reference.arena_index()) as u64
}

fn scoped(program: &TypedTrees, payload: &Payload, query: usize) -> u64 {
    let (state, symbol) = &payload.large_targets[query % payload.large_targets.len()];
    let reference = program
        .declared_binding_type_by_symbol(black_box(state), black_box(*symbol))
        .expect("one exact body declaration");
    black_box(reference.arena_index()) as u64
}

fn run(shape: Shape, mode: &str, work: usize) {
    let payload = payload(shape);
    let mut elapsed = 0;
    let mut checksum = 0;
    match mode {
        "cold_root" | "cold_seal" => {
            for query in 0..work {
                let program = payload.program.clone();
                let started = Instant::now();
                if mode == "cold_seal" {
                    let program = FrozenTypedTrees::from(program);
                    checksum += global(&program, &payload, query);
                    elapsed += started.elapsed().as_nanos();
                    black_box(program);
                } else {
                    checksum += global(&program, &payload, query);
                    elapsed += started.elapsed().as_nanos();
                    black_box(program);
                }
            }
        }
        "cold_selected" => {
            for _ in 0..work {
                let program = payload.program.clone();
                let started = Instant::now();
                for query in 0..payload.large_targets.len() {
                    checksum += scoped(&program, &payload, query);
                }
                elapsed += started.elapsed().as_nanos();
                black_box(program);
            }
        }
        "mutable_scoped" => {
            for query in 0..payload.large_targets.len() {
                scoped(&payload.program, &payload, query);
            }
            let started = Instant::now();
            for query in 0..black_box(work) {
                checksum += scoped(&payload.program, &payload, query);
            }
            elapsed = started.elapsed().as_nanos();
        }
        "frozen_global" | "frozen_scoped" => {
            let program = FrozenTypedTrees::from(payload.program.clone());
            global(&program, &payload, 0);
            scoped(&program, &payload, 0);
            let started = Instant::now();
            for query in 0..black_box(work) {
                checksum += if mode == "frozen_global" {
                    global(&program, &payload, query)
                } else {
                    scoped(&program, &payload, query)
                };
            }
            elapsed = started.elapsed().as_nanos();
        }
        "expression_edits" => {
            let mut program = payload.program.clone();
            global(&program, &payload, 0);
            let started = Instant::now();
            for query in 0..black_box(work) {
                program
                    .expression_table
                    .insert(typed_trees::expression::ExpressionNode::Boolean(true));
                checksum += global(&program, &payload, query);
            }
            elapsed = started.elapsed().as_nanos();
        }
        _ => panic!("unknown owner workload"),
    }
    let expected = work
        * if mode == "cold_selected" {
            shape.large
        } else {
            1
        };
    assert_eq!(checksum, expected as u64);
    println!(
        "{}",
        serde_json::json!({"schema":"omega-timed-work/1","elapsed_ns":elapsed,"work_units":work,"checksum":checksum.to_string()})
    );
}

fn audit(shape: Shape, seal: bool) {
    let payload = payload(shape);
    let prepared = seal.then(|| payload.program.clone());
    COUNTING.store(true, Ordering::Relaxed);
    let before = (
        ALLOCATIONS.load(Ordering::Relaxed),
        BYTES.load(Ordering::Relaxed),
    );
    let frozen = prepared.map(FrozenTypedTrees::from);
    global(frozen.as_deref().unwrap_or(&payload.program), &payload, 0);
    let preparation = (
        ALLOCATIONS.load(Ordering::Relaxed) - before.0,
        BYTES.load(Ordering::Relaxed) - before.1,
    );
    let before = (
        ALLOCATIONS.load(Ordering::Relaxed),
        BYTES.load(Ordering::Relaxed),
    );
    for query in 0..1000 {
        global(
            frozen.as_deref().unwrap_or(&payload.program),
            &payload,
            query,
        );
    }
    let hot = (
        ALLOCATIONS.load(Ordering::Relaxed) - before.0,
        BYTES.load(Ordering::Relaxed) - before.1,
    );
    COUNTING.store(false, Ordering::Relaxed);
    println!(
        "{}",
        serde_json::json!({"tiny":shape.tiny,"large":shape.large,"large_rows":shape.large_rows,"tiny_locals":shape.tiny_locals,"reverse":shape.reverse,"seal":seal,"preparation_requests":preparation.0,"preparation_requested_bytes":preparation.1,"hot_queries":1000,"hot_requests":hot.0,"hot_requested_bytes":hot.1})
    );
}

fn arguments(shape: Shape, mode: &str, work: usize) -> Vec<String> {
    vec![
        "run".into(),
        shape.tiny.to_string(),
        shape.large.to_string(),
        shape.large_rows.to_string(),
        shape.tiny_locals.to_string(),
        if shape.reverse { "reverse" } else { "forward" }.into(),
        mode.into(),
        work.to_string(),
    ]
}

fn plans(directory: &Path, baseline: &Path, candidate: &Path) {
    std::fs::create_dir_all(directory).unwrap();
    let cases = [
        Shape {
            tiny: 18,
            large: 4,
            large_rows: 8,
            tiny_locals: 1,
            reverse: false,
        },
        Shape {
            tiny: 32,
            large: 4,
            large_rows: 64,
            tiny_locals: 1,
            reverse: false,
        },
        Shape {
            tiny: 128,
            large: 4,
            large_rows: 64,
            tiny_locals: 1,
            reverse: false,
        },
        Shape {
            tiny: 1024,
            large: 4,
            large_rows: 64,
            tiny_locals: 1,
            reverse: false,
        },
        Shape {
            tiny: 1024,
            large: 4,
            large_rows: 64,
            tiny_locals: 1,
            reverse: true,
        },
        Shape {
            tiny: 1024,
            large: 4,
            large_rows: 64,
            tiny_locals: 0,
            reverse: true,
        },
        Shape {
            tiny: 32,
            large: 128,
            large_rows: 64,
            tiny_locals: 1,
            reverse: true,
        },
    ];
    let mut files = Vec::new();
    for shape in cases {
        for mode in [
            "cold_root",
            "cold_seal",
            "cold_selected",
            "mutable_scoped",
            "frozen_global",
            "frozen_scoped",
            "expression_edits",
        ] {
            let name = format!(
                "tiny-{}-large-{}-locals-{}-{}-{mode}",
                shape.tiny,
                shape.large,
                shape.tiny_locals,
                if shape.reverse { "reverse" } else { "forward" }
            );
            let work = match mode {
                "cold_root" | "cold_seal" | "cold_selected" => 20,
                "expression_edits" => 200,
                _ => 200_000,
            };
            let command = |program: &Path| serde_json::json!({"program":program,"arguments":arguments(shape,mode,work)});
            let checksum = work
                * if mode == "cold_selected" {
                    shape.large
                } else {
                    1
                };
            let plan = serde_json::json!({"schema":"omega-performance-plan/1","name":name,"working_directory":std::env::current_dir().unwrap(),"inputs":[concat!(env!("CARGO_MANIFEST_DIR"),"/examples/declaration_owner.rs")],"baseline":command(baseline),"candidate":command(candidate),"warmups":2,"pairs":9,"timeout_ms":60000,"measurement":{"kind":"reported_work","stdin":"","work_units":work,"checksum":checksum.to_string()}});
            let filename = format!("{name}.json");
            std::fs::write(
                directory.join(&filename),
                serde_json::to_vec_pretty(&plan).unwrap(),
            )
            .unwrap();
            files.push(filename);
        }
    }
    std::fs::write(
        directory.join("suite.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"schema":"omega-performance-suite/1","plans":files}),
        )
        .unwrap(),
    )
    .unwrap();
}

fn main() {
    let values: Vec<_> = std::env::args().skip(1).collect();
    if values[0] == "plans" {
        plans(
            Path::new(&values[1]),
            Path::new(&values[2]),
            Path::new(&values[3]),
        );
        return;
    }
    let shape = Shape {
        tiny: values[1].parse().unwrap(),
        large: values[2].parse().unwrap(),
        large_rows: values[3].parse().unwrap(),
        tiny_locals: values[4].parse().unwrap(),
        reverse: values[5] == "reverse",
    };
    if values[0] == "audit" {
        audit(shape, values[6] == "seal");
    } else {
        run(shape, &values[6], values[7].parse().unwrap());
    }
}
