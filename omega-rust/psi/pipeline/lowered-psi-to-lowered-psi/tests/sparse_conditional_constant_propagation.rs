//! Stage-level coverage for the exact `SparseConditionalConstantPropagation`
//! rule through the public `run_psi_optimization` entrance. Machine-level
//! boundary cases a validated module cannot express live beside the rewrite
//! in `src/sparse_conditional_constant_propagation/folding.rs`.

use crate::common;

use common::{copy_fixture, sccp_fixture, value};
use lowered_psi_to_lowered_psi::{PsiOptimizationStageError, run_psi_optimization};
use optimization::{PsiOptimization, PsiOptimizationSelections};
use terminal_psi::{OperationKind, Terminator};

fn selections() -> PsiOptimizationSelections {
    PsiOptimizationSelections::new([PsiOptimization::SparseConditionalConstantPropagation]).unwrap()
}

#[test]
fn selected_sccp_folds_literal_resolved_leaves_transitively() {
    let lowered = sccp_fixture();
    let optimized =
        run_psi_optimization(lowered.clone(), selections()).expect("constant folding executes");
    let machine = &optimized.lowered().semantic_module.machines[0];
    let entry = &machine.blocks[0];

    let expected = [
        (
            value(12),
            OperationKind::IntegerConstant {
                value: semantic_vocabulary::IntegerValue::Signed(5),
            },
        ),
        (
            value(13),
            OperationKind::IntegerConstant {
                value: semantic_vocabulary::IntegerValue::Signed(15),
            },
        ),
        (value(14), OperationKind::BooleanConstant { value: true }),
        (value(15), OperationKind::BooleanConstant { value: false }),
    ];
    for (result, kind) in &expected {
        let operation = entry
            .operations
            .iter()
            .find(|operation| operation.result.scalar().is_some_and(|r| r.id == *result))
            .expect("folded operation keeps its position");
        assert_eq!(
            &operation.kind, kind,
            "{result:?} folds to its literal denotation"
        );
    }
    assert_eq!(
        entry.operations.len(),
        6,
        "folding rewrites in place: no operation is added or removed"
    );
    assert_eq!(
        optimized.lowered().semantic_module.machines[0]
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .map(|operation| operation.id)
            .collect::<Vec<_>>(),
        lowered.semantic_module.machines[0]
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .map(|operation| operation.id)
            .collect::<Vec<_>>(),
        "every operation identity survives in order"
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
    terminal_verifier::validate_sparse_conditional_constant_propagation(
        &lowered.semantic_module,
        &optimized.lowered().semantic_module,
    )
    .expect("the independent check accepts the executed rewrite");
}

#[test]
fn opaque_operands_and_control_flow_are_retained() {
    let lowered = sccp_fixture();
    let optimized = run_psi_optimization(lowered, selections()).expect("constant folding executes");
    let machine = &optimized.lowered().semantic_module.machines[0];

    let Terminator::Conditional { condition, .. } = &machine.blocks[0].terminator else {
        panic!("the entry keeps its conditional")
    };
    assert_eq!(
        *condition,
        value(15),
        "the folded v15 still feeds the conditional: branch resolution is the \
         control-flow rule's job, not this rule's"
    );
    for (position, expected) in [(1usize, value(12)), (2, value(13))] {
        let operation = &machine.blocks[position].operations[0];
        assert!(
            matches!(&operation.kind, OperationKind::WrappingIntegerAdd { left, .. }
                if *left == value(1)),
            "the v1-parameter add keeps its operation kind; only the literal \
             operand position could carry {expected:?}"
        );
    }
    assert_eq!(
        machine.blocks.len(),
        4,
        "no block is removed or rewired by this rule"
    );
}

#[test]
fn proof_bearing_closure_remains_unchanged() {
    let mut lowered = sccp_fixture();
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
fn independent_check_rejects_wrong_and_unjustified_folds() {
    let before = sccp_fixture().semantic_module;

    // A fold to the wrong denotation is not the constant computation.
    let mut wrong_value = before.clone();
    wrong_value.machines[0].blocks[0].operations[2].kind = OperationKind::IntegerConstant {
        value: semantic_vocabulary::IntegerValue::Signed(6),
    };
    assert!(matches!(
        terminal_verifier::validate_sparse_conditional_constant_propagation(&before, &wrong_value),
        Err(terminal_verifier::SparseConditionalConstantPropagationRewriteError::ChangedMachine(_))
    ));

    // v20 reads the opaque machine parameter: folding it is unjustified even
    // though one operand is a literal.
    let mut unjustified = before.clone();
    unjustified.machines[0].blocks[1].operations[0].kind = OperationKind::IntegerConstant {
        value: semantic_vocabulary::IntegerValue::Signed(7),
    };
    assert!(matches!(
        terminal_verifier::validate_sparse_conditional_constant_propagation(&before, &unjustified),
        Err(terminal_verifier::SparseConditionalConstantPropagationRewriteError::ChangedMachine(_))
    ));

    // Folding is in place only: removing the foldable v15 and reading v14
    // directly keeps the module valid but is a structural change this rule
    // cannot justify.
    let mut removed = before.clone();
    removed.machines[0].blocks[0].operations.remove(5);
    removed.machines[0].blocks[0].terminator = Terminator::Conditional {
        condition: value(14),
        when_true: common::successor(2, common::block_id(2), vec![]),
        when_false: common::successor(3, common::block_id(3), vec![]),
    };
    assert!(matches!(
        terminal_verifier::validate_sparse_conditional_constant_propagation(&before, &removed),
        Err(terminal_verifier::SparseConditionalConstantPropagationRewriteError::ChangedMachine(_))
    ));

    // A rewired terminator leaves b3 unreachable and fails validation itself.
    let mut rewired = before.clone();
    rewired.machines[0].blocks[0].terminator = Terminator::Jump {
        edge: semantic_vocabulary::EdgeId::new(9).unwrap(),
        target: common::block_id(2),
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    };
    assert!(matches!(
        terminal_verifier::validate_sparse_conditional_constant_propagation(&before, &rewired),
        Err(terminal_verifier::SparseConditionalConstantPropagationRewriteError::InvalidModule(_))
    ));
}

#[test]
fn independent_check_rejects_a_changed_proof_question() {
    let mut lowered = sccp_fixture();
    lowered.semantic_module.machines[0]
        .contract
        .ensures
        .push(terminal_psi::ContractClause {
            obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
            proposition: semantic_vocabulary::Proposition::Truth,
        });
    let mut after = lowered.semantic_module.clone();
    after.machines[0].contract.ensures.clear();
    assert!(matches!(
        terminal_verifier::validate_sparse_conditional_constant_propagation(
            &lowered.semantic_module,
            &after
        ),
        Err(
            terminal_verifier::SparseConditionalConstantPropagationRewriteError::ChangedProofQuestion
        )
    ));
}

#[test]
fn no_constant_workload_records_the_identity() {
    // Boundary coverage against a sibling fixture: no literal-resolved leaf
    // exists, so the selected rule leaves the module untouched.
    let lowered = copy_fixture();
    let optimized =
        run_psi_optimization(lowered.clone(), selections()).expect("constant folding executes");
    assert_eq!(optimized.lowered(), &lowered);
    assert_eq!(
        optimized.execution().input_semantic(),
        optimized.execution().output_semantic()
    );
}

#[test]
fn foldable_leaves_survive_when_constant_propagation_is_not_selected() {
    // Disabled coverage: the foldable workload under a CopyPropagation-only
    // selection keeps every leaf unfolded — constant folding is not selected.
    let lowered = sccp_fixture();
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
fn optimized_output_is_a_legal_second_input_and_reaches_a_fixed_point() {
    let lowered = sccp_fixture();
    let first = run_psi_optimization(lowered, selections()).expect("first run executes");
    let second = run_psi_optimization(first.lowered().clone(), selections())
        .expect("the published artifact re-enters the stage");
    assert_eq!(
        second.lowered(),
        first.lowered(),
        "a second selection changes nothing: every resolved leaf is folded"
    );
    assert_eq!(
        second.execution().input_semantic(),
        second.execution().output_semantic(),
        "the fixed-point run records an identity"
    );
}

#[test]
fn execution_is_deterministic_across_runs() {
    let first = run_psi_optimization(sccp_fixture(), selections()).unwrap();
    let second = run_psi_optimization(sccp_fixture(), selections()).unwrap();
    assert_eq!(first, second, "identical input and selection must agree");
}

#[test]
fn sccp_retains_every_debug_subject_and_updates_semantic_identity() {
    let mut lowered = sccp_fixture();
    common::with_debug_sites(
        &mut lowered,
        &[
            terminal_psi::DebugSubject::Value(value(12)),
            terminal_psi::DebugSubject::Value(value(15)),
            terminal_psi::DebugSubject::Operation(common::operation_id(13)),
            terminal_psi::DebugSubject::Machine(common::machine_id(1)),
        ],
    );
    let sites = lowered
        .debug_map
        .as_ref()
        .unwrap()
        .sites
        .iter()
        .map(|site| site.subject)
        .collect::<Vec<_>>();
    let optimized = run_psi_optimization(lowered, selections()).unwrap();
    let debug = optimized.lowered().debug_map.as_ref().unwrap();
    assert_eq!(
        debug
            .sites
            .iter()
            .map(|site| site.subject)
            .collect::<Vec<_>>(),
        sites,
        "folded operations keep every identity: no debug site is pruned"
    );
    assert_eq!(
        debug.semantic,
        terminal_codec::terminal_psi_identity(&optimized.lowered().semantic_module).unwrap(),
        "the sidecar semantic identity tracks the rewritten module"
    );
}

#[test]
fn structurally_invalid_inputs_fail_before_rewrite() {
    let mut no_machines = sccp_fixture();
    no_machines.semantic_module.machines.clear();
    assert!(matches!(
        run_psi_optimization(no_machines, selections()),
        Err(PsiOptimizationStageError::InvalidModule(
            terminal_verifier::ModuleError::EmptyModule
        ))
    ));

    let mut bad_target = sccp_fixture();
    let Terminator::Jump { target, .. } =
        &mut bad_target.semantic_module.machines[0].blocks[1].terminator
    else {
        panic!("b2 is a jump")
    };
    *target = common::block_id(99);
    assert!(matches!(
        run_psi_optimization(bad_target, selections()),
        Err(PsiOptimizationStageError::InvalidModule(_))
    ));

    let mut bad_debug = sccp_fixture();
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
