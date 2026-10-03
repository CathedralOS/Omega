//! Stage-level coverage for the exact `SparseConditionalConstantPropagation`
//! rule through the public `run_psi_optimization` entrance. Machine-level
//! boundary cases a validated module cannot express live beside the rewrite
//! in `src/sparse_conditional_constant_propagation/folding.rs`.

use crate::common;

use common::{copy_fixture, i32_argument, interpret, publish, sccp_fixture, value};
use lowered_psi_to_lowered_psi::run_psi_optimization;
use optimization::{PsiOptimization, PsiOptimizationSelections};
use terminal_psi::{OperationKind, Terminator};

fn selections() -> PsiOptimizationSelections {
    PsiOptimizationSelections::new([PsiOptimization::SparseConditionalConstantPropagation]).unwrap()
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
        erased_proof_arguments: Vec::new(),
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
fn the_executed_fold_publishes_and_interprets_identically() {
    // Downstream-publication coverage: the folded result crosses Terminal
    // publication's own entrance, the manifest binds the executed output
    // identity, and the canonical bytes admit and run the authored result.
    let lowered = sccp_fixture();
    let baseline =
        run_psi_optimization(lowered.clone(), PsiOptimizationSelections::default()).unwrap();
    let original = publish(&baseline);
    let optimized =
        run_psi_optimization(lowered.clone(), selections()).expect("constant folding executes");
    assert_ne!(
        optimized.execution().input_semantic(),
        optimized.execution().output_semantic(),
        "the fold is a real rewrite, not an identity this leg restates"
    );
    terminal_verifier::validate_sparse_conditional_constant_propagation(
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
    // `v15` resolves to false, so the authored result is `argument + v13`:
    // `10 + 15 = 25` before and after every leaf folds to its literal.
    let arguments = [i32_argument(10)];
    let expected = interpret(&original, &arguments);
    let actual = interpret(&published, &arguments);
    assert_eq!(
        expected,
        terminal_interpreter::TerminalExecutionResult::Scalar(i32_argument(25)),
        "the unoptimized publication returns the authored result"
    );
    assert_eq!(
        actual, expected,
        "the published fold interprets the same result"
    );
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
