// Shared by the allocation-counting and transparent-System timing examples.

use semantic_vocabulary::{IdTable, Proposition, PropositionContext, ValueId};
use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;
use terminal_check_tables::values::ValueRow;

struct MeasureAllocations<const TRACK: bool>;
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static REQUESTED: AtomicUsize = AtomicUsize::new(0);
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
#[global_allocator]
static ALLOCATOR: MeasureAllocations<COUNT_ALLOCATIONS> = MeasureAllocations;

fn allocated(bytes: usize) {
    ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
    REQUESTED.fetch_add(bytes, Ordering::Relaxed);
    let live = LIVE.fetch_add(bytes, Ordering::Relaxed) + bytes;
    PEAK.fetch_max(live, Ordering::Relaxed);
}

// SAFETY: Every request and deallocation is forwarded unchanged to System.
// Counters retain only byte counts; they never inspect allocation contents.
unsafe impl<const TRACK: bool> GlobalAlloc for MeasureAllocations<TRACK> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if TRACK && !pointer.is_null() {
            allocated(layout.size());
        }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        if TRACK {
            LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        }
        unsafe { System.dealloc(pointer, layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let result = unsafe { System.realloc(pointer, layout, size) };
        if TRACK && !result.is_null() {
            LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
            allocated(size);
        }
        result
    }
}

struct Input {
    context: PropositionContext,
    assumptions: Vec<Proposition>,
    axioms: Vec<Proposition>,
    values: IdTable<ValueId, ValueRow>,
    goal: Proposition,
    expected: Option<proof_admission::ProofNode>,
}

fn inputs(path: &std::path::Path) -> Vec<Input> {
    let text = std::fs::read_to_string(path).unwrap();
    let package = semantic_vocabulary::PackageKeyIdentity::from_digest([7; 32]).unwrap();
    let mut sources = source::SourceMap::default();
    let source = sources
        .add_checked_instance(
            path.into(),
            text.as_str(),
            path.parent().unwrap().into(),
            Some(package),
            source::SourceOrigin::User,
            source::SourceResolutionStratum::Base,
            source::DependencyScope::Product,
        )
        .source_id;
    let tokens = source_files_to_tokens::Lexer::new(&text)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees_with_id(source, &tokens).unwrap();
    let mut request = syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax);
    request.sources = Some(Arc::new(sources));
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(request).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let entry = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "main")
        .unwrap()
        .symbol;
    let mut elaboration = typed_trees_to_terminal_psi::elaborate_for(
        &typed,
        typed_trees_to_terminal_psi::Compilation {
            declaration_metadata: &[],
            entries: &[entry],
            subjects: &[Some(package)],
            purpose: source::DependencyScope::Product,
            selected: &[],
            bindings: &[],
            supplies: &[],
            calling: None,
            providers: None,
        },
        terminal_codec::TerminalPublication::Invocation,
    )
    .unwrap();
    let discharge = terminal_psi_to_terminal_psi::discharge(
        &mut elaboration,
        &mut terminal_psi_to_terminal_psi::ProofSearchActivity::default(),
    )
    .unwrap();
    let mut inputs = Vec::new();
    for (machine, result) in discharge.machines.iter() {
        let tables = elaboration.tables.machines.get(machine).unwrap();
        let context = PropositionContext::from_value_types(
            tables
                .values
                .iter()
                .filter(|(_, row)| row.rigid.is_none())
                .map(|(id, row)| (id, row.scalar_type)),
        )
        .unwrap();
        for (_, verdict) in result.verdicts.iter() {
            let verdict = verdict.as_ref().unwrap();
            let Some(evidence) = verdict.certificate() else {
                continue;
            };
            let propositions = |rows: &[terminal_check_tables::KnowledgeId]| {
                rows.iter()
                    .map(|id| {
                        let knowledge = tables.knowledge.get(*id).unwrap();
                        tables
                            .propositions
                            .get(knowledge.proposition)
                            .unwrap()
                            .clone()
                    })
                    .collect()
            };
            inputs.push(Input {
                context: context.clone(),
                assumptions: propositions(&evidence.assumptions),
                axioms: propositions(&evidence.axioms),
                values: tables.values.clone(),
                goal: evidence.goal.clone(),
                expected: Some(evidence.certificate.proof.clone()),
            });
        }
    }
    assert!(!inputs.is_empty());
    inputs
}

fn shape(p: &Proposition, depth: usize, count: &mut usize, width: &mut usize, deepest: &mut usize) {
    *count += 1;
    *deepest = (*deepest).max(depth);
    if let Proposition::Conjunction(parts) = p {
        *width = (*width).max(parts.len());
        for part in parts {
            shape(part, depth + 1, count, width, deepest);
        }
    }
}

fn conjunction_control(name: &str) -> Input {
    use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ScalarTerm, ScalarType};
    use terminal_check_tables::values::ValueOrigin;
    let integer = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let mut values = IdTable::default();
    let value = values.push(ValueRow {
        scalar_type,
        origin: ValueOrigin::MachineParameter { position: 0 },
        rigid: None,
    });
    let relation = |literal| {
        Proposition::LessOrEqual(
            ScalarTerm::value(value, scalar_type),
            ScalarTerm::integer(integer, IntegerValue::Unsigned(literal)).unwrap(),
        )
    };
    let goal = relation(17);
    let assumption = if name == "wide64" {
        Proposition::Conjunction((17..81).map(relation).collect())
    } else {
        let mut nested = goal.clone();
        for literal in 18..26 {
            nested = Proposition::Conjunction(vec![relation(literal), nested]);
        }
        nested
    };
    Input {
        context: PropositionContext::from_value_types([(value, scalar_type)]).unwrap(),
        assumptions: vec![assumption],
        axioms: Vec::new(),
        values,
        goal,
        expected: None,
    }
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    let timing_mode = args.get(3).map(String::as_str);
    let inputs = if matches!(args[1].as_str(), "wide64" | "nested8") {
        vec![conjunction_control(&args[1])]
    } else {
        inputs(std::path::Path::new(&args[1]))
    };
    let repeats: usize = args.get(2).map_or(100, |text| text.parse().unwrap());
    for (index, input) in inputs.iter().enumerate() {
        let mut activity = prove::ProofSearchActivity::default();
        let mut prover = prove::Prover::new(
            &input.context,
            &input.assumptions,
            &input.axioms,
            &input.values,
            &mut activity,
        );
        let proof = prover.prove(&input.goal).unwrap();
        if let Some(expected) = &input.expected {
            assert_eq!(&proof, expected, "selection changed in context {index}");
        }
        let accepted = proof_admission::accept_certificate(
            &input.context,
            &input.goal,
            &input.assumptions,
            &input.axioms,
            &proof,
        )
        .unwrap();
        let rendered = format!("{proof:?}");
        let proof_nodes = rendered.matches("ProofNode {").count();
        let fingerprint = rendered.bytes().fold(0xcbf29ce484222325u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
        });
        let acceptance_fingerprint = format!("{accepted:?}")
            .bytes()
            .fold(0xcbf29ce484222325u64, |hash, byte| {
                (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
            });
        let (mut count, mut width, mut depth) = (0, 0, 0);
        for p in input.assumptions.iter().chain(&input.axioms) {
            shape(p, 0, &mut count, &mut width, &mut depth);
        }
        let before = LIVE.load(Ordering::Relaxed);
        let prepared = known::Known::new(
            &input.context,
            &input.assumptions,
            &input.axioms,
            &input.values,
        );
        let retained_bytes = LIVE.load(Ordering::Relaxed) - before;
        let prepared_nodes = format!("{prepared:?}").matches("ProofNode {").count();
        if timing_mode.is_none() {
            println!(
                "CONTEXT {{\"index\":{index},\"assumptions\":{},\"axioms\":{},\"nodes\":{count},\"width\":{width},\"depth\":{depth},\"facts\":{},\"equations\":{},\"prepared_nodes\":{prepared_nodes},\"selected_nodes\":{proof_nodes},\"used_assumptions\":{},\"used_axioms\":{},\"proof_fingerprint\":\"{fingerprint:016x}\",\"acceptance_fingerprint\":\"{acceptance_fingerprint:016x}\",\"retained_bytes\":{retained_bytes}}}",
                input.assumptions.len(),
                input.axioms.len(),
                prepared.facts.len(),
                prepared.equations.len(),
                accepted.assumptions.len(),
                accepted.semantic_axioms.len()
            );
        }
    }
    for mode in ["prepare", "prove"] {
        if timing_mode.is_some_and(|selected| selected != mode) {
            continue;
        }
        ALLOCATIONS.store(0, Ordering::Relaxed);
        REQUESTED.store(0, Ordering::Relaxed);
        let initial_live = LIVE.load(Ordering::Relaxed);
        PEAK.store(initial_live, Ordering::Relaxed);
        let started = Instant::now();
        let mut checksum = 0usize;
        for _ in 0..repeats {
            for input in &inputs {
                if mode == "prepare" {
                    let known = known::Known::new(
                        &input.context,
                        &input.assumptions,
                        &input.axioms,
                        &input.values,
                    );
                    checksum += black_box(known.facts.len() + known.equations.len());
                    black_box(&known);
                } else {
                    let mut activity = prove::ProofSearchActivity::default();
                    let mut prover = prove::Prover::new(
                        &input.context,
                        &input.assumptions,
                        &input.axioms,
                        &input.values,
                        &mut activity,
                    );
                    let proof = prover.prove(&input.goal).unwrap();
                    checksum += usize::from(black_box(&proof.conclusion) == &input.goal);
                    black_box(&proof);
                }
            }
        }
        let elapsed = started.elapsed().as_nanos();
        let allocations = ALLOCATIONS.load(Ordering::Relaxed);
        let requested = REQUESTED.load(Ordering::Relaxed);
        let peak = PEAK.load(Ordering::Relaxed) - initial_live;
        assert_eq!(LIVE.load(Ordering::Relaxed), initial_live);
        if timing_mode.is_some() {
            println!(
                "{{\"schema\":\"omega-timed-work/1\",\"elapsed_ns\":{elapsed},\"work_units\":{},\"checksum\":\"{checksum}\"}}",
                inputs.len() * repeats
            );
        } else {
            println!(
                "MEASURE {{\"mode\":\"{mode}\",\"contexts\":{},\"repeats\":{repeats},\"ns\":{elapsed},\"allocations\":{allocations},\"requested_bytes\":{requested},\"peak_increment_bytes\":{peak},\"checksum\":{checksum}}}",
                inputs.len()
            );
        }
    }
}
