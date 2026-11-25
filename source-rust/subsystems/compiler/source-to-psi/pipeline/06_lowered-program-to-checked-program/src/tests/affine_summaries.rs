//! Psi 06 walks each machine's affine ownership in several modes, and the
//! walks over one module share callee summaries (`AffineSummaries`) and,
//! per machine, one descriptor graph (`MachineAffine`). Sharing must not
//! change any answer: each walk gives exactly what it gives alone, whatever
//! order the module's machines are walked in.

use super::*;
use terminal_semantics::plain_affine::{AffineSummaries, MachineAffine};

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
    let walks = MachineAffine::new(module, machine, summaries).unwrap();
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
