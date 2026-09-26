//! Stage-level coverage for the exact `GlobalValueNumbering` rule through
//! the public `run_psi_optimization` entrance. Machine-level boundary cases a
//! validated module cannot express live beside the rewrite in
//! `src/global_value_numbering/equivalents.rs`.

use crate::common;

use common::{
    block, block_id, boolean, conditional, edge, i32, jump, lowered, machine, operation,
    operation_id, return_value, successor, value, with_debug_sites,
};
use lowered_psi::LoweredPsi;
use lowered_psi_to_lowered_psi::{PsiOptimizationStageError, run_psi_optimization};
use optimization::{PsiOptimization, PsiOptimizationSelections};
use terminal_psi::{DebugSubject, OperationKind, TerminalMachineResult, Terminator};
use terminal_verifier::{GlobalValueNumberingRewriteError, validate_global_value_numbering};

fn selections() -> PsiOptimizationSelections {
    PsiOptimizationSelections::new([PsiOptimization::GlobalValueNumbering]).unwrap()
}

/// Global-value-numbering fixture. One machine, one diamond, one merge:
///
/// ```text
/// b1 (entry): v10 = v1 + v1; v11 = v1 + v1; v12 = v1 + v1;
///             cond v2 ──e1:t──▶ b2 ──e2:f──▶ b3
/// b2: v20 = v1 + v1; v21 = v20 * v1 ──e3:[v21, v20]──▶ ┐
/// b3: v30 = v1 + v1; v31 = v30 * v1 ──e4:[v31, v11]──▶ b4(v40, v41)
/// b4: v44 = v40 + v41 → return v44
/// ```
///
/// `v11` and `v12` are same-block duplicates of `v10`; `v20`/`v30` repeat the
/// same computation in blocks `b1` dominates. After operand resolution `v21`
/// and `v31` compute the identical `v10 * v1`, but neither sibling block
/// dominates the other, so both survive — the boundary case lives inside the
/// positive fixture. The `v11` edge argument and `v20` operand uses exercise
/// substitution at both sites.
pub fn gvn_fixture() -> LoweredPsi {
    let (b1, b2, b3, b4) = (block_id(1), block_id(2), block_id(3), block_id(4));
    let (v1, v2) = (value(1), value(2));
    let v11 = value(11);
    let (v20, v21) = (value(20), value(21));
    let (v30, v31) = (value(30), value(31));
    let v44 = value(44);
    let add_v1_v1 = |ordinal: u64, result| {
        operation(
            ordinal,
            result,
            OperationKind::WrappingIntegerAdd {
                left: v1,
                right: v1,
            },
        )
    };
    lowered(vec![machine(
        1,
        vec![i32(1), boolean(2)],
        TerminalMachineResult::Scalar(i32(9)),
        b1,
        vec![
            block(
                1,
                Vec::new(),
                vec![
                    add_v1_v1(10, i32(10)),
                    add_v1_v1(11, i32(11)),
                    add_v1_v1(12, i32(12)),
                ],
                conditional(v2, successor(1, b2, vec![]), successor(2, b3, vec![])),
            ),
            block(
                2,
                Vec::new(),
                vec![
                    add_v1_v1(20, i32(20)),
                    operation(
                        21,
                        i32(21),
                        OperationKind::WrappingIntegerMultiply {
                            left: v20,
                            right: v1,
                        },
                    ),
                ],
                jump(3, b4, vec![v21, v20]),
            ),
            block(
                3,
                Vec::new(),
                vec![
                    add_v1_v1(30, i32(30)),
                    operation(
                        31,
                        i32(31),
                        OperationKind::WrappingIntegerMultiply {
                            left: v30,
                            right: v1,
                        },
                    ),
                ],
                jump(4, b4, vec![v31, v11]),
            ),
            block(
                4,
                vec![i32(40), i32(41)],
                vec![operation(
                    40,
                    i32(44),
                    OperationKind::WrappingIntegerAdd {
                        left: value(40),
                        right: value(41),
                    },
                )],
                return_value(5, v44),
            ),
        ],
    )])
}

/// The negative boundary alone: identical computations live only in sibling
/// blocks, so no survivor ever dominates a duplicate.
///
/// ```text
/// b1 (entry): cond v2 ──e1:t──▶ b2 ──e2:f──▶ b3
/// b2: v20 = v1 + v1 ──e3:[v20]──▶ ┐
/// b3: v30 = v1 + v1 ──e4:[v30]──▶ b4(v40) → return v40
/// ```
pub fn sibling_gvn_fixture() -> LoweredPsi {
    let (b1, b2, b3, b4) = (block_id(1), block_id(2), block_id(3), block_id(4));
    let (v1, v2) = (value(1), value(2));
    let (v20, v30, v40) = (value(20), value(30), value(40));
    let add = |ordinal: u64, result| {
        operation(
            ordinal,
            result,
            OperationKind::WrappingIntegerAdd {
                left: v1,
                right: v1,
            },
        )
    };
    lowered(vec![machine(
        1,
        vec![i32(1), boolean(2)],
        TerminalMachineResult::Scalar(i32(9)),
        b1,
        vec![
            block(
                1,
                Vec::new(),
                Vec::new(),
                conditional(v2, successor(1, b2, vec![]), successor(2, b3, vec![])),
            ),
            block(
                2,
                Vec::new(),
                vec![add(20, i32(20))],
                jump(3, b4, vec![v20]),
            ),
            block(
                3,
                Vec::new(),
                vec![add(30, i32(30))],
                jump(4, b4, vec![v30]),
            ),
            block(4, vec![i32(40)], Vec::new(), return_value(5, v40)),
        ],
    )])
}

#[test]
fn selected_numbering_collapses_dominating_duplicates_to_the_canonical_survivor() {
    let lowered = gvn_fixture();
    let optimized =
        run_psi_optimization(lowered.clone(), selections()).expect("value numbering executes");
    let machine = &optimized.lowered().semantic_module.machines[0];

    assert_eq!(
        machine.blocks[0]
            .operations
            .iter()
            .map(|operation| operation.id)
            .collect::<Vec<_>>(),
        vec![operation_id(10)],
        "v11 and v12 repeat v10 in its own block: the first computation survives"
    );
    assert_eq!(
        machine.blocks[1]
            .operations
            .iter()
            .map(|operation| operation.id)
            .collect::<Vec<_>>(),
        vec![operation_id(21)],
        "dominated v20 collapses; the resolved v10 * v1 is a new computation"
    );
    assert_eq!(
        machine.blocks[1].operations[0].kind,
        OperationKind::WrappingIntegerMultiply {
            left: value(10),
            right: value(1)
        },
        "the surviving operand resolves the collapsed duplicate"
    );
    let Terminator::Jump { arguments, .. } = &machine.blocks[1].terminator else {
        panic!("b2 keeps its jump")
    };
    assert_eq!(
        arguments,
        &[value(21), value(10)],
        "the removed v20 substitutes the canonical survivor at the edge"
    );
    assert_eq!(
        machine.blocks[2]
            .operations
            .iter()
            .map(|operation| operation.id)
            .collect::<Vec<_>>(),
        vec![operation_id(31)],
        "dominated v30 collapses; sibling v31 computes the same value but stays"
    );
    let Terminator::Jump { arguments, .. } = &machine.blocks[2].terminator else {
        panic!("b3 keeps its jump")
    };
    assert_eq!(
        arguments,
        &[value(31), value(10)],
        "the removed v11 substitutes at the edge"
    );
    assert_eq!(
        optimized.lowered().semantic_module.machines[0].blocks[3],
        lowered.semantic_module.machines[0].blocks[3],
        "the merge block is untouched"
    );
    assert_ne!(
        optimized.execution().input_semantic(),
        optimized.execution().output_semantic(),
        "the rewrite is recorded as a semantic change"
    );
    assert_eq!(
        optimized.execution().input_proof(),
        optimized.execution().output_proof(),
        "the proof bundle is byte-identical"
    );
    validate_global_value_numbering(
        &lowered.semantic_module,
        &optimized.lowered().semantic_module,
    )
    .expect("the independent check accepts the executed rewrite");
}

#[test]
fn sibling_duplicates_without_dominance_are_retained() {
    // Negative boundary: identical computations exist only across the
    // dominance cut, so the rule publishes a validated identity.
    let lowered = sibling_gvn_fixture();
    let optimized =
        run_psi_optimization(lowered.clone(), selections()).expect("value numbering executes");
    assert_eq!(
        optimized.lowered(),
        &lowered,
        "v20 and v30 compute the same value, but neither block dominates the other"
    );
    assert_eq!(
        optimized.execution().input_semantic(),
        optimized.execution().output_semantic()
    );
}

#[test]
fn duplicates_survive_when_global_value_numbering_is_not_selected() {
    // Disabled coverage: the duplicate workload under a CopyPropagation-only
    // selection keeps every operation — collapsing equivalents is not selected.
    let lowered = gvn_fixture();
    let optimized = run_psi_optimization(
        lowered.clone(),
        PsiOptimizationSelections::new([PsiOptimization::CopyPropagation]).unwrap(),
    )
    .expect("the stage executes");
    assert_eq!(optimized.lowered(), &lowered);
    assert_eq!(
        optimized.execution().input_semantic(),
        optimized.execution().output_semantic()
    );
}

#[test]
fn proof_bearing_closure_remains_unchanged() {
    let mut lowered = gvn_fixture();
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
    let before = gvn_fixture().semantic_module;

    // v31 is eligible, but its only equal computation sits in sibling b2,
    // which does not dominate b3: removing it has no justifying leader.
    let mut no_leader = before.clone();
    let b3 = &mut no_leader.machines[0].blocks[2];
    b3.operations
        .retain(|operation| operation.id != operation_id(31));
    let Terminator::Jump { arguments, .. } = &mut b3.terminator else {
        panic!("b3 is a jump")
    };
    *arguments = vec![value(10), value(10)];
    assert!(matches!(
        validate_global_value_numbering(&before, &no_leader),
        Err(GlobalValueNumberingRewriteError::MissingDominatingEquivalent(
            operation
        )) if operation == operation_id(31)
    ));

    // An obligation-bearing checked operation is never an eligible duplicate:
    // removing it is not this rule's rewrite at all.
    let mut with_check = before.clone();
    with_check.machines[0].blocks[0].operations.insert(
        1,
        operation(
            15,
            i32(15),
            OperationKind::ExactIntegerSubtract {
                left: value(1),
                right: value(1),
                obligation: common::obligation(7),
            },
        ),
    );
    let mut removed_check = with_check.clone();
    removed_check.machines[0].blocks[0]
        .operations
        .retain(|operation| operation.id != operation_id(15));
    assert!(matches!(
        validate_global_value_numbering(&with_check, &removed_check),
        Err(
            GlobalValueNumberingRewriteError::RemovedIneligibleOperation(operation)
        ) if operation == operation_id(15)
    ));

    // Collapsing the duplicates but substituting a different value than the
    // canonical survivor is drift the replayed reconstruction catches.
    let mut wrong_substitute = before.clone();
    wrong_substitute.machines[0].blocks[0]
        .operations
        .retain(|operation| operation.id != operation_id(11) && operation.id != operation_id(12));
    wrong_substitute.machines[0].blocks[1]
        .operations
        .retain(|operation| operation.id != operation_id(20));
    wrong_substitute.machines[0].blocks[1].operations[0].kind =
        OperationKind::WrappingIntegerMultiply {
            left: value(10),
            right: value(1),
        };
    let Terminator::Jump { arguments, .. } = &mut wrong_substitute.machines[0].blocks[1].terminator
    else {
        panic!("b2 is a jump")
    };
    *arguments = vec![value(21), value(21)];
    wrong_substitute.machines[0].blocks[2]
        .operations
        .retain(|operation| operation.id != operation_id(30));
    wrong_substitute.machines[0].blocks[2].operations[0].kind =
        OperationKind::WrappingIntegerMultiply {
            left: value(10),
            right: value(1),
        };
    let Terminator::Jump { arguments, .. } = &mut wrong_substitute.machines[0].blocks[2].terminator
    else {
        panic!("b3 is a jump")
    };
    *arguments = vec![value(31), value(10)];
    assert!(matches!(
        validate_global_value_numbering(&before, &wrong_substitute),
        Err(GlobalValueNumberingRewriteError::ChangedMachine(machine))
            if machine == common::machine_id(1)
    ));

    // A machine added to the module is a structural change, not a collapse.
    let mut added = before.clone();
    added.machines.push(machine(
        2,
        Vec::new(),
        TerminalMachineResult::Unit,
        block_id(10),
        vec![block(
            10,
            Vec::new(),
            Vec::new(),
            Terminator::ReturnUnit {
                edge: edge(10),
                trivial_affine_discards: Vec::new(),
            },
        )],
    ));
    assert!(matches!(
        validate_global_value_numbering(&before, &added),
        Err(GlobalValueNumberingRewriteError::ChangedProgramStructure)
    ));
}

#[test]
fn independent_check_rejects_a_changed_proof_question() {
    // A structurally exact collapse still refuses when the reconstructed
    // proof question cannot be carried verbatim: both sides publish the same
    // ensures clause, but the removal changes the axioms it reconstructs.
    let clause = terminal_psi::ContractClause {
        obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
        proposition: semantic_vocabulary::Proposition::Truth,
    };
    let mut before = gvn_fixture();
    before.semantic_module.machines[0]
        .contract
        .ensures
        .push(clause.clone());
    let mut after = run_psi_optimization(gvn_fixture(), selections())
        .unwrap()
        .into_lowered()
        .semantic_module;
    after.machines[0].contract.ensures.push(clause);
    assert!(matches!(
        validate_global_value_numbering(&before.semantic_module, &after),
        Err(GlobalValueNumberingRewriteError::ChangedProofQuestion)
    ));
}

#[test]
fn optimized_output_is_a_legal_second_input_and_reaches_a_fixed_point() {
    let lowered = gvn_fixture();
    let first = run_psi_optimization(lowered, selections()).expect("first run executes");
    // The published artifact is a validated `LoweredPsi`, a legal second
    // input — this is fixed-point coverage, not repeated reconstruction.
    let second = run_psi_optimization(first.lowered().clone(), selections())
        .expect("the published artifact re-enters the stage");
    assert_eq!(
        second.lowered(),
        first.lowered(),
        "a second selection changes nothing: no dominated duplicate remains"
    );
    assert_eq!(
        second.execution().input_semantic(),
        second.execution().output_semantic(),
        "the fixed-point run records an identity"
    );
}

#[test]
fn execution_is_deterministic_across_runs() {
    let first = run_psi_optimization(gvn_fixture(), selections()).unwrap();
    let second = run_psi_optimization(gvn_fixture(), selections()).unwrap();
    assert_eq!(first, second, "identical input and selection must agree");
}

#[test]
fn numbering_prunes_removed_subjects_and_updates_semantic_identity() {
    let mut lowered = gvn_fixture();
    with_debug_sites(
        &mut lowered,
        &[
            DebugSubject::Value(value(11)),
            DebugSubject::Value(value(21)),
            DebugSubject::Operation(operation_id(12)),
            DebugSubject::Operation(operation_id(20)),
            DebugSubject::Operation(operation_id(21)),
            DebugSubject::Machine(common::machine_id(1)),
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
        !subjects.contains(&DebugSubject::Value(value(11)))
            && !subjects.contains(&DebugSubject::Operation(operation_id(12)))
            && !subjects.contains(&DebugSubject::Operation(operation_id(20))),
        "removed result and operation sites are dropped: {subjects:?}"
    );
    assert!(subjects.contains(&DebugSubject::Value(value(21))));
    assert!(subjects.contains(&DebugSubject::Operation(operation_id(21))));
    assert_eq!(
        debug.semantic,
        terminal_codec::terminal_psi_identity(&optimized.lowered().semantic_module).unwrap(),
        "the sidecar semantic identity tracks the rewritten module"
    );
}

#[test]
fn structurally_invalid_inputs_fail_before_rewrite() {
    // Corruption coverage: each malformed carrier fails closed at the
    // module-validation gate before any rewrite decision.
    let mut no_machines = gvn_fixture();
    no_machines.semantic_module.machines.clear();
    assert!(matches!(
        run_psi_optimization(no_machines, selections()),
        Err(PsiOptimizationStageError::InvalidModule(
            terminal_verifier::ModuleError::EmptyModule
        ))
    ));

    let mut bad_target = gvn_fixture();
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

    let mut bad_arity = gvn_fixture();
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

    let mut bad_debug = gvn_fixture();
    with_debug_sites(&mut bad_debug, &[]);
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
fn ranked_machine_renumbers_outside_the_covered_component() {
    // The `Natural` row covers member block b3: its operations stay exact and
    // the duplicate `v21` survives because member contents still name it on
    // the covered backedge. `v22` is an ordinary duplicate outside the
    // covered component and rewrites to `v20`.
    let lowered = common::ranked_cycle_fixture();
    let optimized =
        run_psi_optimization(lowered.clone(), selections()).expect("renumbering executes");
    let machine = &optimized.lowered().semantic_module.machines[0];

    assert_eq!(
        machine.blocks[0]
            .operations
            .iter()
            .map(|operation| operation.id)
            .collect::<Vec<_>>(),
        vec![common::operation_id(20), common::operation_id(21)],
        "v22 is an uncovered duplicate and leaves; member-used v21 keeps its identity"
    );
    let Terminator::Conditional {
        when_true,
        when_false,
        ..
    } = &machine.blocks[0].terminator
    else {
        panic!("the entry keeps its conditional")
    };
    assert_eq!(when_true.arguments, &[value(1), value(20)]);
    assert_eq!(when_false.arguments, &[value(1), value(20)]);

    let member = &machine.blocks[2];
    assert_eq!(
        member
            .parameters
            .iter()
            .map(|parameter| parameter.id)
            .collect::<Vec<_>>(),
        vec![value(10), value(11), value(32)],
        "the covered parameter table stays exact"
    );
    assert_eq!(
        member
            .operations
            .iter()
            .map(|operation| operation.id)
            .collect::<Vec<_>>(),
        vec![
            common::operation_id(40),
            common::operation_id(42),
            common::operation_id(41)
        ],
        "member operations stay exact — even the duplicate constant"
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
