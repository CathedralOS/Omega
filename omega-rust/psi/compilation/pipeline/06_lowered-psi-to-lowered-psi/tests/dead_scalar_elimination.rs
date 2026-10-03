//! Stage-level coverage for the exact `DeadPureScalarElimination` rule
//! through the public `run_psi_optimization` entrance.

use crate::common;

use common::{
    block_id, copy_fixture, dead_scalar_fixture, i32_argument, interpret, operation_id, publish,
    value,
};
use lowered_psi_to_lowered_psi::{PsiOptimizationStageError, run_psi_optimization};
use optimization::{PsiOptimization, PsiOptimizationSelections};
use terminal_psi::{OperationKind, Terminator};
use terminal_verifier::{DeadScalarRewriteError, validate_dead_scalar_elimination};

fn selections() -> PsiOptimizationSelections {
    PsiOptimizationSelections::new([PsiOptimization::DeadPureScalarElimination]).unwrap()
}

#[test]
fn the_executed_elimination_publishes_and_interprets_identically() {
    // Downstream-publication coverage: the pruned result crosses Terminal
    // publication's own entrance, the manifest binds the executed output
    // identity, and the canonical bytes admit and run the authored result.
    let lowered = dead_scalar_fixture();
    let baseline =
        run_psi_optimization(lowered.clone(), PsiOptimizationSelections::default()).unwrap();
    let original = publish(&baseline);
    let optimized = run_psi_optimization(lowered.clone(), selections())
        .expect("dead scalar elimination executes");
    assert_ne!(
        optimized.execution().input_semantic(),
        optimized.execution().output_semantic(),
        "the removal is a real rewrite, not an identity this leg restates"
    );
    validate_dead_scalar_elimination(
        &lowered.semantic_module,
        &optimized.lowered().semantic_module,
    )
    .expect("the independent check accepts the executed rewrite");
    let published = publish(&optimized);
    assert_eq!(
        published.manifest().semantic(),
        optimized.execution().output_semantic(),
        "the manifest binds the executed output identity"
    );
    // The true arm feeds the merge v41 = argument and v42 = 2 while v43, its
    // producers, and the unused entry constant all die: the authored result
    // is `argument + 2` and the removals must not change it.
    let arguments = [
        terminal_interpreter::TerminalScalarValue::Boolean(true),
        i32_argument(5),
    ];
    let expected = interpret(&original, &arguments);
    let actual = interpret(&published, &arguments);
    assert_eq!(
        expected,
        terminal_interpreter::TerminalExecutionResult::Scalar(i32_argument(7)),
        "the unoptimized publication returns the authored result"
    );
    assert_eq!(
        actual, expected,
        "the published elimination interprets the same result"
    );
}

#[test]
fn the_published_elimination_reenters_the_stage_at_its_fixed_point() {
    // Fixed-point coverage: the executed removal's own publication decodes
    // back to a legal stage input, and the same selection finds no remaining
    // dead scalar on it — transitive liveness already settled on the artifact.
    let first = run_psi_optimization(dead_scalar_fixture(), selections()).unwrap();
    assert_ne!(
        first.execution().input_semantic(),
        first.execution().output_semantic(),
        "the first run rewrites, so the refeed is not restating a vacuous identity"
    );
    let refed = common::refeed(&publish(&first));
    let second = run_psi_optimization(refed.clone(), selections())
        .expect("the published artifact re-enters the stage");
    assert_eq!(
        second.lowered(),
        &refed,
        "the elimination finds no remaining dead scalar on its own publication"
    );
    assert_eq!(
        second.execution().input_semantic(),
        second.execution().output_semantic(),
        "the reentered run records the identity: the published module is the fixed point"
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
fn execution_is_deterministic_across_runs() {
    let first = run_psi_optimization(dead_scalar_fixture(), selections()).unwrap();
    let second = run_psi_optimization(dead_scalar_fixture(), selections()).unwrap();
    assert_eq!(first, second);
}

#[test]
fn structurally_invalid_inputs_fail_before_rewrite() {
    // Corruption coverage: each malformed carrier fails closed at the
    // module-validation gate before any rewrite decision.
    let mut no_machines = dead_scalar_fixture();
    no_machines.semantic_module.machines.clear();
    assert!(matches!(
        run_psi_optimization(no_machines, selections()),
        Err(PsiOptimizationStageError::InvalidModule(
            terminal_verifier::ModuleError::EmptyModule
        ))
    ));

    let mut bad_target = dead_scalar_fixture();
    let Terminator::Jump { target, .. } =
        &mut bad_target.semantic_module.machines[0].blocks[1].terminator
    else {
        panic!("b2 is a jump")
    };
    *target = block_id(99);
    assert!(matches!(
        run_psi_optimization(bad_target, selections()),
        Err(PsiOptimizationStageError::InvalidModule(_))
    ));

    let mut bad_arity = dead_scalar_fixture();
    let Terminator::Jump { arguments, .. } =
        &mut bad_arity.semantic_module.machines[0].blocks[1].terminator
    else {
        panic!("b2 is a jump")
    };
    arguments.pop();
    assert!(matches!(
        run_psi_optimization(bad_arity, selections()),
        Err(PsiOptimizationStageError::InvalidModule(_))
    ));

    let mut bad_debug = dead_scalar_fixture();
    common::with_debug_sites(&mut bad_debug, &[]);
    bad_debug
        .debug_map
        .as_mut()
        .unwrap()
        .semantic
        .program_fingerprint = terminal_psi::SemanticFingerprint::from_bytes([0xff; 32]);
    assert!(matches!(
        run_psi_optimization(bad_debug, selections()),
        Err(PsiOptimizationStageError::InvalidDebugMap(_))
    ));
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
