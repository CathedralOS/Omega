//! Psi 06 walks each machine's affine ownership in several modes, and the
//! walks over one module share callee summaries (`AffineSummaries`) and,
//! per machine, one descriptor graph (`MachineAffine`). After
//! `prepare_discharge` the affine check and every machine check also share
//! one `ModuleAnalyses` bundle, which adapts each body's descriptor row once.
//! Sharing must not change any answer: each walk and each machine's closure
//! gives exactly what it gives alone, whatever order the module's machines
//! are walked in, and a module the bundle cannot describe is refused as
//! before.

use super::*;
use terminal_semantics::descriptor_results::DescriptorMachine;
use terminal_semantics::plain_affine::{AffineSummaries, MachineAffine};
use terminal_semantics::{DescriptorRows, ModuleAnalyses};

/// Two callers reach one leaf through a middle machine, and each call
/// returns a view of its caller's storage, so every caller's walk replays
/// the summaries of the machines below it.
const CHAIN: &str = r#"
data Cell [copy] { value: u64; }
data Boxes [copy] { cells: [Cell; 2]; }
machine leaf(boxes: @mutable Boxes) -> @mutable Cell {
    let cells: @mutable [Cell] = @mutable boxes.cells[..];
    @mutable cells[0]
}
machine middle(boxes: @mutable Boxes) -> @mutable Cell { leaf(boxes) }
machine left(boxes: @mutable Boxes) -> @mutable Cell { middle(boxes) }
machine right(boxes: @mutable Boxes) -> @mutable Cell { middle(boxes) }
machine entry() -> u64 {
    let mutable boxes: Boxes;
    let cell: @mutable Cell = left(@mutable boxes);
    cell.value = 7u64;
    let other: @mutable Cell = right(@mutable boxes);
    other.value = 8u64;
    boxes.cells[0].value
}
"#;

fn elaborate(text: &str) -> crate::lowering::Elaboration {
    let typed = super::boundary_calls::typed(text);
    let entry = super::boundary_calls::entry(&typed);
    crate::lowering::elaborate_for(
        &typed,
        crate::lowering::Compilation {
            declaration_metadata: &[],
            entries: &[entry],
            subjects: &[],
            purpose: source_provenance::DependencyScope::Product,
            selected: &[],
            bindings: &[],
            supplies: &[],
            calling: None,
            providers: None,
        },
        terminal_codec::TerminalPublication::Invocation,
    )
    .expect("the call chain lowers")
}

/// Every mode's answer for one machine, from one set of walks.
fn walked(
    module: &terminal_psi::TerminalModule,
    machine: MachineId,
    candidates: &terminal_semantics::reference_ends::Candidates,
    summaries: Option<&AffineSummaries<'_>>,
) -> String {
    answers(
        &MachineAffine::new(module, machine, summaries).unwrap(),
        candidates,
    )
}

fn answers(
    walks: &MachineAffine<'_>,
    candidates: &terminal_semantics::reference_ends::Candidates,
) -> String {
    format!(
        "{:?}\n{:?}\n{:?}\n{:?}\n{:?}",
        walks.plan_reference_ends(candidates),
        walks.verify(),
        walks.verify_claims(),
        walks.plan(),
        walks.plan_claims(),
    )
}

/// The answers of the one-walk entry points, each building its own graph
/// and replaying every callee itself.
fn alone(
    module: &terminal_psi::TerminalModule,
    machine: MachineId,
    candidates: &terminal_semantics::reference_ends::Candidates,
) -> String {
    use terminal_semantics::plain_affine as affine;
    format!(
        "{:?}\n{:?}\n{:?}\n{:?}\n{:?}",
        affine::plan_reference_ends(module, machine, candidates),
        affine::verify_in(module, machine),
        affine::verify_claims_in(module, machine),
        affine::plan_in(module, machine),
        affine::plan_claims_in(module, machine),
    )
}

#[test]
fn shared_affine_walks_answer_as_each_walk_alone() {
    let elaboration = elaborate(CHAIN);
    let module = &elaboration.module;
    let Ok(candidates) = terminal_semantics::reference_ends::candidates(module) else {
        panic!("the chain has reference-end candidates");
    };
    let expected = module
        .machines
        .iter()
        .map(|machine| alone(module, machine.id, candidates.get(machine.id).unwrap()))
        .collect::<Vec<_>>();
    // The machines whose bodies some walk reads besides its own: the
    // callees whose summaries the walks replay.
    let mut callees = module
        .machines
        .iter()
        .filter_map(|machine| terminal_semantics::plain_affine::plan_in(module, machine.id).ok())
        .flat_map(|plan| plan.graphed_machines)
        .collect::<Vec<_>>();
    callees.sort_unstable();
    callees.dedup();
    assert!(
        callees.len() >= 2,
        "the callers' walks read the middle and leaf bodies: {callees:?}"
    );
    let forward = (0..module.machines.len()).collect::<Vec<_>>();
    let reverse = forward.iter().rev().copied().collect::<Vec<_>>();
    for order in [forward, reverse] {
        let summaries = AffineSummaries::new(module);
        for &position in &order {
            let machine = module.machines[position].id;
            assert_eq!(
                walked(
                    module,
                    machine,
                    candidates.get(machine).unwrap(),
                    Some(&summaries)
                ),
                expected[position],
                "machine {machine:?}"
            );
            assert_eq!(
                walked(module, machine, candidates.get(machine).unwrap(), None),
                expected[position],
                "machine {machine:?} without a table"
            );
        }
        // The first walk to reach each callee published its summary for
        // the later callers, which then adopt it.
        for callee in &callees {
            assert!(summaries.holds(*callee), "{callee:?} was not published");
        }
    }
}

#[test]
fn affine_summaries_of_another_module_are_not_shared() {
    let elaboration = elaborate(CHAIN);
    let copy = elaboration.module.clone();
    let Ok(candidates) = terminal_semantics::reference_ends::candidates(&copy) else {
        panic!("the copy has reference-end candidates");
    };
    let foreign = AffineSummaries::new(&elaboration.module);
    for machine in &copy.machines {
        let candidates = candidates.get(machine.id).unwrap();
        assert_eq!(
            walked(&copy, machine.id, candidates, Some(&foreign)),
            alone(&copy, machine.id, candidates),
        );
    }
    // The table belongs to the original module; walks of an equal copy
    // replay alone and publish nothing into it.
    assert!(
        elaboration
            .module
            .machines
            .iter()
            .all(|machine| !foreign.holds(machine.id))
    );
}

#[test]
fn walks_over_the_shared_analyses_answer_as_each_walk_alone() {
    let elaboration = elaborate(CHAIN);
    let module = &elaboration.module;
    let Ok(candidates) = terminal_semantics::reference_ends::candidates(module) else {
        panic!("the chain has reference-end candidates");
    };
    let expected = module
        .machines
        .iter()
        .map(|machine| alone(module, machine.id, candidates.get(machine.id).unwrap()))
        .collect::<Vec<_>>();
    let mut callees = module
        .machines
        .iter()
        .filter_map(|machine| terminal_semantics::plain_affine::plan_in(module, machine.id).ok())
        .flat_map(|plan| plan.graphed_machines)
        .collect::<Vec<_>>();
    callees.sort_unstable();
    callees.dedup();
    assert!(callees.len() >= 2, "{callees:?}");
    let forward = (0..module.machines.len()).collect::<Vec<_>>();
    let reverse = forward.iter().rev().copied().collect::<Vec<_>>();
    for order in [forward, reverse] {
        // As `examine` builds them after `prepare_discharge`.
        let analyses = ModuleAnalyses::new(module).unwrap();
        let summaries = AffineSummaries::new(module);
        let Ok(shared_candidates) =
            terminal_semantics::reference_ends::candidates_sharing_in(&analyses, &summaries)
        else {
            panic!("the shared analyses give the chain's reference-end candidates");
        };
        for &position in &order {
            let machine = module.machines[position].id;
            let candidates = candidates.get(machine).unwrap();
            assert_eq!(shared_candidates.get(machine), Some(candidates));
            let walks = MachineAffine::from_analyses(&analyses, machine, Some(&summaries)).unwrap();
            assert_eq!(
                answers(&walks, candidates),
                expected[position],
                "machine {machine:?}"
            );
            assert_eq!(
                answers(&analyses.affine(machine).unwrap(), candidates),
                expected[position],
                "machine {machine:?} without a summary table"
            );
        }
        // Walks over the shared rows still publish callee summaries for
        // the later callers.
        for callee in &callees {
            assert!(summaries.holds(*callee), "{callee:?} was not published");
        }
    }
}

/// What a descriptor row states, as text, for comparing rows that do not
/// implement equality.
fn shape(row: &DescriptorMachine) -> String {
    format!(
        "{:?} returns {:?} types {:?} entry {:?} flow {:?} cases {:?} {:?} origins {:?} \
         projections {:?} queries {:?} producers {:?} literals {:?} derivations {:?} \
         unresolved {:?} exits {:?} calls {:?}",
        row.id,
        row.returns,
        row.place_types,
        row.entry,
        row.flow.keys().collect::<Vec<_>>(),
        row.case_constructors,
        row.case_successors,
        row.originations,
        row.projections,
        row.contents_queries,
        row.payload_producers,
        row.unsigned_index_literals,
        row.storage_derivations,
        row.unresolved_contents_writes,
        row.normal_exits,
        row.calls.keys().collect::<Vec<_>>(),
    )
}

fn shapes(rows: &DescriptorRows<'_>) -> Vec<String> {
    rows.iter().map(shape).collect()
}

#[test]
fn a_machine_check_without_generic_call_inputs_reads_the_shared_rows() {
    let elaboration = elaborate(CHAIN);
    let module = &elaboration.module;
    let analyses = ModuleAnalyses::new(module).unwrap();
    let mut shared = 0;
    for machine in &module.machines {
        let alone =
            crate::places::reference_graph_with_generic_calls(module, None, machine.id, &[], &[])
                .unwrap();
        assert!(matches!(alone, DescriptorRows::Owned(_)));
        let rows = crate::places::reference_graph_with_generic_calls(
            module,
            Some(&analyses),
            machine.id,
            &[],
            &[],
        )
        .unwrap();
        // A machine with no reference-bearing storage still has no closure.
        if alone.is_empty() {
            assert!(rows.is_empty(), "machine {:?}", machine.id);
            continue;
        }
        assert!(
            matches!(rows, DescriptorRows::Shared { .. }),
            "machine {:?}",
            machine.id
        );
        shared += 1;
        assert_eq!(shapes(&rows), shapes(&alone), "machine {:?}", machine.id);
    }
    assert!(shared >= 2, "the chain's callers check reference closures");
    // Analyses of an equal copy describe other bodies, so the check builds
    // this module's closure itself.
    let copy = module.clone();
    let foreign = ModuleAnalyses::new(&copy).unwrap();
    for machine in &module.machines {
        let rows = crate::places::reference_graph_with_generic_calls(
            module,
            Some(&foreign),
            machine.id,
            &[],
            &[],
        )
        .unwrap();
        assert!(
            matches!(rows, DescriptorRows::Owned(_)),
            "machine {:?}",
            machine.id
        );
    }
}

#[test]
fn a_machine_check_with_nominal_calls_adapts_its_own_rows() {
    let source = "
data Token<const Key: u64> { cells: [u16; 2]; }
machine entry<const N: u64, const M: u64>(token: @Token<N>) -> u64 {
    child(token)
    machine child(token: @Token<N>) -> u64 {
        let cells: @[u16] = @token.cells[..];
        cells.length
    }
}";
    let typed =
        crate::preparation::prepare_typed_trees(super::boundary_calls::typed(source), &[], &[])
            .unwrap();
    let entry = super::boundary_calls::entry(typed.trees());
    let form = crate::lowering::elaborate_generic(typed.trees(), entry).unwrap();
    assert!(!form.nominal_calls.is_empty(), "the call names Token<N>");
    let module = &form.module;
    let analyses = ModuleAnalyses::new(module).unwrap();
    let mut checked = 0;
    for machine in &module.machines {
        let with_calls = |analyses| {
            crate::places::reference_graph_with_generic_calls(
                module,
                analyses,
                machine.id,
                &form.nominal_calls,
                &form.static_schemas,
            )
            .unwrap()
        };
        let alone = with_calls(None);
        let rows = with_calls(Some(&analyses));
        // Rows adapted with the nominal calls are this closure's own; the
        // bundle's rows were adapted without them.
        assert!(
            matches!(rows, DescriptorRows::Owned(_)),
            "machine {:?}",
            machine.id
        );
        assert_eq!(shapes(&rows), shapes(&alone), "machine {:?}", machine.id);
        checked += usize::from(!rows.is_empty());
    }
    assert!(checked >= 1, "a machine borrows the token");
}

#[test]
fn a_module_that_declares_a_machine_twice_keeps_its_refusal() {
    let elaboration = elaborate(CHAIN);
    let mut repeated = elaboration.module.clone();
    let copy = repeated.machines[1].clone();
    repeated.machines.push(copy);
    let Err(error) = ModuleAnalyses::new(&repeated) else {
        panic!("the bundle refuses a repeated machine identity");
    };
    // `examine` refuses with what the affine check's candidates refused
    // with before the bundle was shared: the same machine, operation and
    // detail.
    let Err(refused) = terminal_semantics::reference_ends::candidates_sharing(
        &repeated,
        &AffineSummaries::new(&repeated),
    ) else {
        panic!("the candidates refuse a repeated machine identity");
    };
    assert_eq!(
        (refused.machine, refused.operation, refused.detail),
        (
            error.machine(),
            error.operation(&repeated),
            error.in_module(&repeated)
        )
    );
}
