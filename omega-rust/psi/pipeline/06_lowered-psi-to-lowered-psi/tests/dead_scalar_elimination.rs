//! Stage-level coverage for the exact `DeadPureScalarElimination` rule
//! through the public `run_psi_optimization` entrance.

use crate::common;

use common::{block_id, copy_fixture, dead_scalar_fixture, operation_id, value};
use lowered_psi_to_lowered_psi::{PsiOptimizationStageError, run_psi_optimization};
use optimization::{PsiOptimization, PsiOptimizationSelections};
use terminal_psi::{OperationKind, Terminator};
use terminal_verifier::{DeadScalarRewriteError, validate_dead_scalar_elimination};

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
fn proof_bearing_closure_remains_unchanged() {
    let mut lowered = dead_scalar_fixture();
    lowered.semantic_module.machines[0]
        .contract
        .ensures
        .push(terminal_psi::ContractClause {
            obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
            proposition: semantic_vocabulary::Proposition::Truth,
        });
    let optimized = run_psi_optimization(lowered.clone(), selections())
        .expect("a proof-bearing closure remains unchanged");
    assert_eq!(
        optimized.lowered(),
        &lowered,
        "reconstructed obligations freeze the whole closure until \
         proof-context transport is implemented"
    );
    assert_eq!(
        optimized.execution().input_semantic(),
        optimized.execution().output_semantic(),
        "the frozen run records an identity"
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

#[test]
fn independent_check_rejects_a_changed_proof_question() {
    // A structurally exact removal still refuses when the reconstructed
    // proof question cannot be carried verbatim: both sides publish the same
    // ensures clause, but the removal changes the axioms it reconstructs.
    let clause = terminal_psi::ContractClause {
        obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
        proposition: semantic_vocabulary::Proposition::Truth,
    };
    let mut before = dead_scalar_fixture();
    before.semantic_module.machines[0]
        .contract
        .ensures
        .push(clause.clone());
    let mut after = run_psi_optimization(dead_scalar_fixture(), selections())
        .unwrap()
        .into_lowered()
        .semantic_module;
    after.machines[0].contract.ensures.push(clause);
    assert!(matches!(
        validate_dead_scalar_elimination(&before.semantic_module, &after),
        Err(DeadScalarRewriteError::ChangedProofQuestion)
    ));
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
fn ranked_machine_eliminates_outside_the_covered_component() {
    // The `Natural` row covers member block b3's parameter table and named
    // values. The merge's dead parameter drops with its edge positions, dead
    // producers in the entry die, and the covered block's parameters — even
    // the unused copy-shaped v32 — stay live.
    let lowered = common::ranked_cycle_fixture();
    let optimized =
        run_psi_optimization(lowered.clone(), selections()).expect("elimination executes");
    let machine = &optimized.lowered().semantic_module.machines[0];

    let merge = &machine.blocks[1];
    assert_eq!(
        merge
            .parameters
            .iter()
            .map(|parameter| parameter.id)
            .collect::<Vec<_>>(),
        vec![value(30)],
        "the dead v31 drops outside the covered component"
    );
    let Terminator::Conditional {
        when_true,
        when_false,
        ..
    } = &machine.blocks[0].terminator
    else {
        panic!("the entry keeps its conditional")
    };
    assert_eq!(when_true.arguments, &[value(1)]);
    assert_eq!(when_false.arguments, &[value(1)]);
    assert_eq!(
        machine.blocks[0]
            .operations
            .iter()
            .map(|operation| operation.id)
            .collect::<Vec<_>>(),
        vec![common::operation_id(21)],
        "v20 and v22 die; v21 stays live through the covered component"
    );

    let member = &machine.blocks[2];
    assert_eq!(
        member
            .parameters
            .iter()
            .map(|parameter| parameter.id)
            .collect::<Vec<_>>(),
        vec![value(10), value(11), value(32)],
        "the covered parameter table stays exact even for the unused v32"
    );
    assert_eq!(
        member
            .operations
            .iter()
            .map(|operation| operation.id)
            .collect::<Vec<_>>(),
        vec![common::operation_id(40), common::operation_id(41)],
        "the duplicate constant inside the member is dead like any other"
    );
    let Terminator::Conditional { when_true, .. } = &member.terminator else {
        panic!("the member keeps its conditional")
    };
    assert_eq!(when_true.arguments, &[value(41), value(11), value(21)]);
    assert_eq!(
        machine.ranked_scc, lowered.semantic_module.machines[0].ranked_scc,
        "the ranking row is untouched"
    );
}
