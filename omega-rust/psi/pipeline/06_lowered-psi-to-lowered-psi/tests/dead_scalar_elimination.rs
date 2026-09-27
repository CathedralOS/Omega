//! Stage-level coverage for the exact `DeadPureScalarElimination` rule
//! through the public `run_psi_optimization` entrance.

use crate::common;

use common::{block_id, copy_fixture, dead_scalar_fixture, operation_id, value};
use lowered_psi_to_lowered_psi::run_psi_optimization;
use optimization::{PsiOptimization, PsiOptimizationSelections};
use terminal_psi::{OperationKind, Terminator};
use terminal_verifier::{DeadScalarRewriteError, validate_dead_scalar_elimination};

fn selections() -> PsiOptimizationSelections {
    PsiOptimizationSelections::new([PsiOptimization::DeadPureScalarElimination]).unwrap()
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

#[test]
fn a_dead_checked_operation_keeps_the_whole_module_unchanged() {
    // Negative boundary: elimination removes only unconditionally-total
    // scalars. The checked subtraction's obligation makes the module
    // proof-bearing, so the refused rewrite freezes the closure — even the
    // unrelated dead total work stays until proof-context transport exists.
    let mut lowered = dead_scalar_fixture();
    lowered.semantic_module.machines[0].blocks[0]
        .operations
        .insert(
            1,
            common::operation(
                11,
                common::i32(60),
                OperationKind::ExactIntegerSubtract {
                    left: value(2),
                    right: value(2),
                    obligation: common::obligation(7),
                },
            ),
        );
    let optimized =
        run_psi_optimization(lowered.clone(), selections()).expect("elimination executes");
    assert_eq!(
        optimized.lowered(),
        &lowered,
        "an operation-site obligation freezes the whole machine"
    );
    assert_eq!(
        optimized.execution().input_semantic(),
        optimized.execution().output_semantic()
    );
}

#[test]
fn independent_check_rejects_unjustified_removals() {
    let lowered = dead_scalar_fixture();
    let before = &lowered.semantic_module;
    let optimized = run_psi_optimization(lowered.clone(), selections()).unwrap();
    let after = &optimized.lowered().semantic_module;
    validate_dead_scalar_elimination(before, after)
        .expect("the independent check accepts the executed rewrite");

    // A surviving operation whose contents changed is not a removal.
    let mut changed_survivor = after.clone();
    changed_survivor.machines[0].blocks[3].operations[0].kind = OperationKind::WrappingIntegerAdd {
        left: value(41),
        right: value(2),
    };
    assert!(matches!(
        validate_dead_scalar_elimination(before, &changed_survivor),
        Err(DeadScalarRewriteError::ChangedSurvivingOperation(operation))
            if operation == operation_id(40)
    ));

    // An obligation-bearing checked operation is never a dead total scalar:
    // removing it is not this rule's rewrite at all.
    let mut with_check = before.clone();
    with_check.machines[0].blocks[0].operations.insert(
        1,
        common::operation(
            11,
            common::i32(60),
            OperationKind::ExactIntegerSubtract {
                left: value(2),
                right: value(2),
                obligation: common::obligation(7),
            },
        ),
    );
    let mut removed_check = with_check.clone();
    removed_check.machines[0].blocks[0]
        .operations
        .retain(|operation| operation.id != operation_id(11));
    assert!(matches!(
        validate_dead_scalar_elimination(&with_check, &removed_check),
        Err(DeadScalarRewriteError::RemovedNonTotalOperation(operation))
            if operation == operation_id(11)
    ));

    // Reordering the surviving parameter list is a block-shape change, not a
    // dead-parameter removal.
    let mut reordered = after.clone();
    reordered.machines[0].blocks[3].parameters.swap(0, 1);
    assert!(matches!(
        validate_dead_scalar_elimination(before, &reordered),
        Err(DeadScalarRewriteError::ChangedBlockParameters(block))
            if block == block_id(4)
    ));

    // Edge arguments that drop the right count but bind different values than
    // the retained positions are drift, not the removed parameter's slots.
    let mut swapped_arguments = after.clone();
    let Terminator::Jump { arguments, .. } =
        &mut swapped_arguments.machines[0].blocks[1].terminator
    else {
        panic!("b2 is a jump")
    };
    *arguments = vec![value(20), value(2)];
    assert!(matches!(
        validate_dead_scalar_elimination(before, &swapped_arguments),
        Err(DeadScalarRewriteError::ChangedEdgeArguments(edge))
            if edge == common::edge(4)
    ));

    // A machine added to the module is a structural change, not a removal.
    let mut added = after.clone();
    added.machines.push(common::machine(
        2,
        Vec::new(),
        terminal_psi::TerminalMachineResult::Unit,
        block_id(10),
        vec![common::block(
            10,
            Vec::new(),
            Vec::new(),
            Terminator::ReturnUnit {
                edge: common::edge(10),
                trivial_affine_discards: Vec::new(),
            },
        )],
    ));
    assert!(matches!(
        validate_dead_scalar_elimination(before, &added),
        Err(DeadScalarRewriteError::ChangedProgramStructure)
    ));
}
