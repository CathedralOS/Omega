//! Stage-level coverage for the exact `DeadPureScalarElimination` rule
//! through the public `run_psi_optimization` entrance.

mod common;

use common::{copy_fixture, dead_scalar_fixture, value};
use lowered_psi_to_lowered_psi::run_psi_optimization;
use optimization::{PsiOptimization, PsiOptimizationSelections};
use terminal_psi::Terminator;

fn selections() -> PsiOptimizationSelections {
    PsiOptimizationSelections::new([PsiOptimization::DeadPureScalarElimination]).unwrap()
}

#[test]
fn selected_elimination_removes_dead_results_parameters_and_edge_arguments() {
    let lowered = dead_scalar_fixture();
    let optimized =
        run_psi_optimization(lowered.clone(), selections()).expect("elimination executes");
    let machine = &optimized.lowered().semantic_module.machines[0];

    assert!(
        machine.blocks[0].operations.is_empty(),
        "the unused v10 constant dies in the entry block"
    );
    assert_eq!(
        machine.blocks[1]
            .operations
            .iter()
            .map(|operation| operation.id.get())
            .collect::<Vec<_>>(),
        vec![20],
        "the producer feeding the dead parameter dies transitively"
    );
    assert_eq!(
        machine.blocks[2]
            .operations
            .iter()
            .map(|operation| operation.id.get())
            .collect::<Vec<_>>(),
        vec![30]
    );
    let merge = &machine.blocks[3];
    assert_eq!(
        merge.parameters.iter().map(|p| p.id).collect::<Vec<_>>(),
        vec![value(41), value(42)],
        "only the unused v43 parameter is removed"
    );
    let Terminator::Jump { arguments, .. } = &machine.blocks[1].terminator else {
        panic!("b2 keeps its jump")
    };
    assert_eq!(
        arguments,
        &[value(2), value(20)],
        "the edge argument position for v43 drops"
    );
    let Terminator::Jump { arguments, .. } = &machine.blocks[2].terminator else {
        panic!("b3 keeps its jump")
    };
    assert_eq!(arguments, &[value(2), value(30)]);
    let Terminator::Return {
        value: returned, ..
    } = &merge.terminator
    else {
        panic!("b4 keeps its return")
    };
    assert_eq!(*returned, value(44), "the live result chain is unchanged");
    assert_ne!(
        optimized.execution().input_semantic(),
        optimized.execution().output_semantic()
    );
    assert_eq!(
        optimized.lowered().proof_bundle,
        lowered.proof_bundle,
        "the proof bundle is byte-identical"
    );
}

#[test]
fn copy_parameters_survive_when_copy_propagation_is_not_selected() {
    // Disabled coverage: the uniform-binding v41 copy remains a parameter
    // under a DeadPureScalarElimination-only selection even though it is live.
    let lowered = dead_scalar_fixture();
    let optimized =
        run_psi_optimization(lowered.clone(), selections()).expect("elimination executes");
    let merge = &optimized.lowered().semantic_module.machines[0].blocks[3];
    assert!(
        merge.parameters.iter().any(|p| p.id == value(41)),
        "v41 stays a parameter: collapsing copies is not selected"
    );
}

#[test]
fn everything_live_input_is_an_identity_rewrite() {
    // Boundary: the copy fixture has no dead scalar — the rule fires and
    // publishes a validated identity rather than trimming live values.
    let lowered = copy_fixture();
    let optimized =
        run_psi_optimization(lowered.clone(), selections()).expect("elimination executes");
    assert_eq!(
        optimized.lowered().semantic_module,
        lowered.semantic_module,
        "no live value, parameter, or edge argument is removed"
    );
    assert_eq!(
        optimized.execution().input_semantic(),
        optimized.execution().output_semantic(),
        "the no-work run records an identity"
    );
}

#[test]
fn optimized_output_is_a_legal_second_input_and_reaches_a_fixed_point() {
    let first = run_psi_optimization(dead_scalar_fixture(), selections()).unwrap();
    let second = run_psi_optimization(first.lowered().clone(), selections())
        .expect("the published artifact re-enters the stage");
    assert_eq!(second.lowered(), first.lowered());
    assert_eq!(
        second.execution().input_semantic(),
        second.execution().output_semantic()
    );
}

#[test]
fn execution_is_deterministic_across_runs() {
    let first = run_psi_optimization(dead_scalar_fixture(), selections()).unwrap();
    let second = run_psi_optimization(dead_scalar_fixture(), selections()).unwrap();
    assert_eq!(first, second);
}

#[test]
fn elimination_prunes_removed_value_and_operation_debug_sites() {
    let mut lowered = dead_scalar_fixture();
    common::with_debug_sites(
        &mut lowered,
        &[
            terminal_psi::DebugSubject::Value(value(43)),
            terminal_psi::DebugSubject::Value(value(41)),
            terminal_psi::DebugSubject::Operation(common::operation_id(21)),
            terminal_psi::DebugSubject::Operation(common::operation_id(20)),
        ],
    );
    let optimized = run_psi_optimization(lowered, selections()).unwrap();
    let debug = optimized.lowered().debug_map.as_ref().unwrap();
    let subjects = debug
        .sites
        .iter()
        .map(|site| site.subject)
        .collect::<Vec<_>>();
    assert!(
        !subjects.contains(&terminal_psi::DebugSubject::Value(value(43)))
            && !subjects.contains(&terminal_psi::DebugSubject::Operation(
                common::operation_id(21)
            )),
        "removed parameter and operation sites are dropped: {subjects:?}"
    );
    assert!(subjects.contains(&terminal_psi::DebugSubject::Value(value(41))));
    assert!(subjects.contains(&terminal_psi::DebugSubject::Operation(
        common::operation_id(20)
    )));
    assert_eq!(
        debug.semantic,
        terminal_codec::terminal_psi_identity(&optimized.lowered().semantic_module).unwrap()
    );
}
