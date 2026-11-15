//! Portable declaration-lookup payloads. `plans <directory>` writes inputs for
//! omega-bench; optional comma-separated sizes and a kind filter select cases.
//! `audit <bindings|fields> <size> [queries]` reports allocation/work counts.
//! Generated graphs isolate lookup cost and do not represent whole compilations.

use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use symbols::{SymbolHandle, SymbolKind, SymbolNameRef, SymbolTableBuilder};
use typed_trees::TypedTrees;
use typed_trees::data::{DataDefinition, DataField, DataMember};
use typed_trees::signature::StateParameter;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TableLocalData};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

struct CountedAllocator;
static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);

// Only this isolated example counts allocation requests. Deallocation retains
// the system allocator's original pointer/layout contract.
unsafe impl GlobalAlloc for CountedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(size as u64, Ordering::Relaxed);
        unsafe { System.realloc(pointer, layout, size) }
    }
}

#[global_allocator]
static ALLOCATOR: CountedAllocator = CountedAllocator;

struct Payload {
    program: TypedTrees,
    states: Vec<State>,
    symbols: Vec<SymbolHandle>,
}

fn payload(kind: &str, size: usize, alternating: bool) -> Payload {
    let mut symbols = SymbolTableBuilder::default();
    let queried: Vec<_> = (0..size.max(1) * if alternating { 2 } else { 1 })
        .map(|_| {
            symbols.insert_root(
                if kind == "fields" {
                    SymbolKind::Field
                } else {
                    SymbolKind::Local
                },
                SymbolNameRef::Static("value"),
            )
        })
        .collect();
    let mut program = TypedTrees {
        symbols: symbols.finish(),
        ..Default::default()
    };
    let reference = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let mut states = Vec::new();
    if kind == "bindings" {
        for queried in queried.chunks(size.max(1)) {
            let parameters = program.state_parameters.insert_many([StateParameter {
                symbol: queried[0],
                type_reference: reference,
                ..Default::default()
            }]);
            let statement_nodes =
                program
                    .statement_table
                    .insert_statements(queried.iter().skip(1).map(|&symbol| {
                        StatementNode::LocalData(TableLocalData {
                            symbol,
                            type_reference: reference,
                            ..Default::default()
                        })
                    }));
            states.push(State {
                parameters,
                statement_nodes,
                ..Default::default()
            });
        }
    } else {
        assert_eq!(kind, "fields");
        for &symbol in &queried {
            let members = program
                .data_members
                .insert_many([DataMember::Field(DataField {
                    symbol,
                    type_reference: reference,
                    ..Default::default()
                })]);
            program.push_data_definition(DataDefinition {
                members,
                ..Default::default()
            });
        }
    }
    Payload {
        program,
        states,
        symbols: queried,
    }
}

fn lookup<const COUNT: bool>(
    payload: &Payload,
    kind: &str,
    candidate: bool,
    body: usize,
    symbol: SymbolHandle,
    visits: &mut u64,
) -> Option<TypeReferenceHandle> {
    let program = &payload.program;
    if candidate {
        return if kind == "bindings" {
            program.declared_binding_type_by_symbol(&payload.states[body], symbol)
        } else {
            program
                .unique_declared_field_by_symbol(symbol)
                .map(|field| field.type_reference)
        };
    }
    if !program.symbols.symbols().nodes().is_valid(symbol) {
        return None;
    }
    let mut found = None;
    if kind == "bindings" {
        for (binder, reference) in program
            .state_parameters(&payload.states[body])
            .iter()
            .map(|p| (p.symbol, p.type_reference))
            .chain(
                program
                    .statement_table
                    .statements(payload.states[body].statement_nodes)
                    .iter()
                    .filter_map(StatementNode::as_local_data)
                    .map(|p| (p.symbol, p.type_reference)),
            )
        {
            if COUNT {
                *visits += 1;
            }
            if binder == symbol {
                if found.is_some() {
                    return None;
                }
                found = Some(reference);
            }
        }
        found.filter(|reference| {
            program
                .type_reference_table
                .contains_type_reference(*reference)
        })
    } else {
        for field in program
            .data_definitions()
            .iter()
            .flat_map(|data| program.data_member_fields(program.data_members(data)))
        {
            if COUNT {
                *visits += 1;
            }
            if field.symbol == symbol {
                if found.is_some() {
                    return None;
                }
                found = Some(field.type_reference);
            }
        }
        found
    }
}

fn selected(payload: &Payload, kind: &str, query: usize, body: usize) -> SymbolHandle {
    if kind == "fields" {
        payload.symbols[0]
    } else {
        let size = payload.symbols.len() / payload.states.len();
        payload.symbols[body * size + (query * 17) % size]
    }
}

fn audit(kind: &str, size: usize, queries: usize) {
    let payload = payload(kind, size, false);
    let symbol = payload.symbols[0];
    let mut visits = 0;
    let baseline = lookup::<true>(&payload, kind, false, 0, symbol, &mut visits);
    let before = (
        ALLOCATIONS.load(Ordering::Relaxed),
        BYTES.load(Ordering::Relaxed),
    );
    assert_eq!(
        lookup::<false>(&payload, kind, true, 0, symbol, &mut 0),
        baseline
    );
    let cold = (
        ALLOCATIONS.load(Ordering::Relaxed) - before.0,
        BYTES.load(Ordering::Relaxed) - before.1,
    );
    visits = 0;
    for query in 0..queries {
        black_box(
            lookup::<true>(
                &payload,
                kind,
                false,
                0,
                selected(&payload, kind, query, 0),
                &mut visits,
            )
            .unwrap(),
        );
    }
    let before = (
        ALLOCATIONS.load(Ordering::Relaxed),
        BYTES.load(Ordering::Relaxed),
    );
    for query in 0..queries {
        black_box(
            lookup::<false>(
                &payload,
                kind,
                true,
                0,
                selected(&payload, kind, query, 0),
                &mut 0,
            )
            .unwrap(),
        );
    }
    let hot = (
        ALLOCATIONS.load(Ordering::Relaxed) - before.0,
        BYTES.load(Ordering::Relaxed) - before.1,
    );
    println!(
        "{}",
        serde_json::json!({"kind":kind,"rows":size,"baseline_hot_row_visits":visits,"candidate_input_rows":size,"cold_allocation_requests":cold.0,"cold_requested_bytes":cold.1,"hot_queries":queries,"hot_allocation_requests":hot.0,"hot_requested_bytes":hot.1})
    );
}

fn run(kind: &str, candidate: bool, size: usize, queries: usize, mode: &str) {
    let cold = mode == "cold";
    let payload = payload(kind, size, mode == "alternating");
    let mut visits = 0;
    if !cold {
        lookup::<false>(
            &payload,
            kind,
            candidate,
            0,
            payload.symbols[0],
            &mut visits,
        )
        .unwrap();
    }
    let mut elapsed = 0;
    let mut checksum = 0;
    if cold {
        for query in 0..queries {
            let fresh = Payload {
                program: payload.program.clone(),
                states: payload.states.clone(),
                symbols: payload.symbols.clone(),
            };
            let symbol = selected(&fresh, kind, query, 0);
            let started = Instant::now();
            let reference = lookup::<false>(
                black_box(&fresh),
                kind,
                candidate,
                0,
                black_box(symbol),
                &mut visits,
            )
            .unwrap();
            elapsed += started.elapsed().as_nanos();
            checksum += black_box(reference.arena_index()) as u64;
        }
    } else {
        let started = Instant::now();
        for query in 0..black_box(queries) {
            let body = if mode == "alternating" { query % 2 } else { 0 };
            let reference = lookup::<false>(
                black_box(&payload),
                kind,
                candidate,
                body,
                black_box(selected(&payload, kind, query, body)),
                &mut visits,
            )
            .unwrap();
            checksum += black_box(reference.arena_index()) as u64;
        }
        elapsed = started.elapsed().as_nanos();
    }
    println!(
        "{}",
        serde_json::json!({"schema":"omega-timed-work/1","elapsed_ns":elapsed,"work_units":queries,"checksum":checksum.to_string()})
    );
}

fn plans(directory: &std::path::Path, sizes: &[usize], selected_kind: &str) {
    std::fs::create_dir_all(directory).unwrap();
    let executable = std::env::current_exe().unwrap();
    let owner = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let inputs: Vec<_> = [
        "examples/declaration_lookups.rs",
        "src/declared_bindings.rs",
        "src/declaration_tables_and_index.rs",
        "src/calls_and_control_flow.rs",
        "src/lib.rs",
    ]
    .map(|path| owner.join(path))
    .into_iter()
    .collect();
    let mut all = Vec::new();
    for kind in ["bindings", "fields"] {
        if selected_kind != "both" && selected_kind != kind {
            continue;
        }
        for &size in sizes {
            for mode in ["hot", "cold", "alternating"] {
                if kind == "fields" && mode == "alternating" {
                    continue;
                }
                let name = format!("{kind}-{size}-{mode}");
                let queries = if mode == "cold" {
                    2000
                } else {
                    200_000 / (size / 16).max(1)
                };
                let command = |role| serde_json::json!({"program":executable,"arguments":["run",kind,role,size.to_string(),queries.to_string(),mode]});
                let plan = serde_json::json!({
                    "schema":"omega-performance-plan/1","name":name,
                    "working_directory":std::env::current_dir().unwrap(),"inputs":inputs,
                    "baseline":command("baseline"),"candidate":command("candidate"),
                    "warmups":2,"pairs":15,"timeout_ms":60000,
                    "measurement":{"kind":"reported_work","stdin":"","work_units":queries,"checksum":queries.to_string()}
                });
                let filename = format!("{name}.json");
                std::fs::write(
                    directory.join(&filename),
                    serde_json::to_vec_pretty(&plan).unwrap(),
                )
                .unwrap();
                all.push(filename);
            }
        }
    }
    std::fs::write(
        directory.join("suite.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"schema":"omega-performance-suite/1","plans":all}),
        )
        .unwrap(),
    )
    .unwrap();
}

fn summarize(directory: &std::path::Path) {
    let mut reports = std::fs::read_dir(directory)
        .unwrap()
        .filter_map(|entry| {
            let path = entry.unwrap().path().join("report.json");
            path.is_file().then(|| {
                serde_json::from_slice::<serde_json::Value>(&std::fs::read(path).unwrap()).unwrap()
            })
        })
        .collect::<Vec<_>>();
    reports.sort_by(|left, right| left["name"].as_str().cmp(&right["name"].as_str()));
    println!(
        "case,baseline_ns,candidate_ns,paired_change_percent,bootstrap95_percent,interpretation"
    );
    for report in reports {
        let comparison = &report["comparison"];
        println!(
            "{},{},{},{},\"{}\",{}",
            report["name"],
            comparison["baseline"]["median"],
            comparison["candidate"]["median"],
            comparison["paired_percent_change"]["median"],
            comparison["paired_percent_change_bootstrap_95"],
            comparison["interpretation"]
        );
    }
}

fn construction(locals: usize, queries: usize, broad: bool) {
    let mut payload = payload("bindings", locals + 1, false);
    let mut state = payload.states[0].clone();
    let symbol = *payload.symbols.last().unwrap();
    let mut statements = payload
        .program
        .statement_table
        .statements(state.statement_nodes)
        .to_vec();
    statements.push(StatementNode::Call(Default::default()));
    state.statement_nodes = payload
        .program
        .statement_table
        .insert_statements(statements);
    let call = state.statement_nodes.handle_at(locals as u32);
    payload
        .program
        .declared_binding_type_by_symbol(&state, symbol)
        .unwrap();
    let before = (
        ALLOCATIONS.load(Ordering::Relaxed),
        BYTES.load(Ordering::Relaxed),
    );
    let mut checksum = 0;
    for _ in 0..queries {
        let call = if broad {
            let StatementNode::Call(call) = payload.program.statement_table.statement_mut(call)
            else {
                panic!("call annotation")
            };
            call
        } else {
            payload.program.statement_table.call_mut(call).unwrap()
        };
        call.authored_machine_application = Default::default();
        checksum += payload
            .program
            .declared_binding_type_by_symbol(&state, symbol)
            .unwrap()
            .arena_index() as usize;
    }
    let requests = ALLOCATIONS.load(Ordering::Relaxed) - before.0;
    let requested_bytes = BYTES.load(Ordering::Relaxed) - before.1;
    assert_eq!(checksum, queries);
    println!(
        "{}",
        serde_json::json!({"locals":locals,"queries":queries,"broad":broad,"requests":requests,"requested_bytes":requested_bytes,"checksum":checksum})
    );
}

fn main() {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    match arguments[0].as_str() {
        "construction" => construction(
            arguments[1].parse().unwrap(),
            arguments[2].parse().unwrap(),
            arguments.get(3).is_some_and(|value| value == "broad"),
        ),
        "audits" => {
            for kind in ["bindings", "fields"] {
                for size in [4, 64, 1024] {
                    for queries in [1, 4, 16, 64, 1000] {
                        audit(kind, size, queries);
                    }
                }
            }
        }
        "summary" => summarize(std::path::Path::new(&arguments[1])),
        "plans" => {
            let sizes = arguments.get(2).map_or_else(
                || vec![1, 4, 8, 16, 32, 64, 256, 1024],
                |value| value.split(',').map(|size| size.parse().unwrap()).collect(),
            );
            plans(
                std::path::Path::new(&arguments[1]),
                &sizes,
                arguments.get(3).map_or("both", String::as_str),
            );
        }
        "audit" => audit(
            &arguments[1],
            arguments[2].parse().unwrap(),
            arguments
                .get(3)
                .map_or(1000, |value| value.parse().unwrap()),
        ),
        "run" => run(
            &arguments[1],
            arguments[2] == "candidate",
            arguments[3].parse().unwrap(),
            arguments[4].parse().unwrap(),
            &arguments[5],
        ),
        _ => panic!("expected plans, audit or run"),
    }
}
