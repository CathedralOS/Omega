//! Pre-Terminal Psi optimization entrance regressions.

use super::*;
use optimization::{PsiOptimization, PsiOptimizationSelections};

#[test]
fn empty_selection_executes_validated_identity_before_publication() {
    let lowered = lower_machine(&hard_root_checked_fixture(), "example::Root::enter")
        .expect("fixture lowers");
    let expected = lowered.clone();
    let selections = PsiOptimizationSelections::default();
    let selection_identity = selections.identity();

    let optimized = run_psi_optimization(lowered, selections).expect("identity stage executes");

    assert_eq!(optimized.lowered(), &expected);
    assert!(optimized.selections().is_empty());
    assert_eq!(optimized.execution().selection(), selection_identity);
    assert_eq!(
        optimized.execution().input_semantic(),
        optimized.execution().output_semantic()
    );
    assert_eq!(
        optimized.execution().input_proof(),
        optimized.execution().output_proof()
    );
    let artifact = finalize_terminal_artifact(&optimized).expect("optimized result publishes");
    assert_eq!(
        artifact.manifest().semantic(),
        optimized.execution().output_semantic()
    );
}

#[test]
fn every_unported_nonempty_selection_fails_closed() {
    let lowered = lower_machine(&hard_root_checked_fixture(), "example::Root::enter")
        .expect("fixture lowers");

    for optimization in PsiOptimization::ALL {
        let selections = PsiOptimizationSelections::new([optimization]).unwrap();
        assert!(matches!(
            run_psi_optimization(lowered.clone(), selections),
            Err(PsiOptimizationStageError::UnsupportedSelection(actual))
                if actual == optimization
        ));
    }
}

#[test]
fn invalid_input_fails_before_selected_rule_dispatch() {
    let mut lowered = lower_machine(&hard_root_checked_fixture(), "example::Root::enter")
        .expect("fixture lowers");
    lowered.semantic_module.machines.clear();
    let selections = PsiOptimizationSelections::new([PsiOptimization::ControlFlowCleanup]).unwrap();

    assert!(matches!(
        run_psi_optimization(lowered, selections),
        Err(PsiOptimizationStageError::InvalidModule(
            terminal_verifier::ModuleError::EmptyModule
        ))
    ));
}
