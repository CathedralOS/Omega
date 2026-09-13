//! Stage-entrance coverage shared by every exact rule: the validated identity
//! transformation, selection dispatch, canonical order, and fail-closed
//! handling of unsupported or malformed inputs.

mod common;

use common::{copy_fixture, dead_scalar_fixture, minimal_unit_lowered, value};
use lowered_psi_to_lowered_psi::{PsiOptimizationStageError, run_psi_optimization};
use optimization::{PsiOptimization, PsiOptimizationSelections};
use terminal_psi::Terminator;

#[test]
fn empty_selection_executes_validated_identity() {
    let lowered = minimal_unit_lowered();
    let optimized = run_psi_optimization(lowered.clone(), PsiOptimizationSelections::default())
        .expect("the identity stage executes");
    assert_eq!(optimized.lowered(), &lowered);
    assert!(optimized.selections().is_empty());
    assert_eq!(
        optimized.execution().input_semantic(),
        optimized.execution().output_semantic()
    );
    assert_eq!(
        optimized.execution().input_proof(),
        optimized.execution().output_proof()
    );
}

#[test]
fn selections_without_a_stage_rule_execute_the_identity_for_that_rule() {
    // Disabled coverage at the stage level: an input that carries both a copy
    // parameter and a dead scalar is unchanged under the empty selection.
    for fixture in [copy_fixture(), dead_scalar_fixture()] {
        let optimized =
            run_psi_optimization(fixture.clone(), PsiOptimizationSelections::default()).unwrap();
        assert_eq!(optimized.lowered(), &fixture);
    }
}

#[test]
fn unported_selections_fail_closed_instead_of_recording_identity() {
    for optimization in PsiOptimization::ALL {
        if matches!(
            optimization,
            PsiOptimization::CopyPropagation | PsiOptimization::DeadPureScalarElimination
        ) {
            continue;
        }
        let selections = PsiOptimizationSelections::new([optimization]).unwrap();
        assert_eq!(
            run_psi_optimization(minimal_unit_lowered(), selections),
            Err(PsiOptimizationStageError::UnsupportedSelection(
                optimization
            )),
            "{optimization:?} must not be recorded as an executed identity"
        );
    }
}

#[test]
fn invalid_input_fails_before_any_selection_dispatch() {
    // Corruption coverage: input validation precedes rule dispatch, so even a
    // valid selection cannot rescue a malformed carrier.
    let mut lowered = copy_fixture();
    lowered.semantic_module.machines.clear();
    for optimization in PsiOptimization::ALL {
        let selections = PsiOptimizationSelections::new([optimization]).unwrap();
        assert!(matches!(
            run_psi_optimization(lowered.clone(), selections),
            Err(PsiOptimizationStageError::InvalidModule(
                terminal_verifier::ModuleError::EmptyModule
            ))
        ));
    }
}

#[test]
fn combined_selection_runs_both_rules_in_canonical_order() {
    let lowered = dead_scalar_fixture();
    let selections = PsiOptimizationSelections::new([
        PsiOptimization::DeadPureScalarElimination,
        PsiOptimization::CopyPropagation,
    ])
    .unwrap();
    let optimized = run_psi_optimization(lowered, selections).unwrap();
    let machine = &optimized.lowered().semantic_module.machines[0];
    let merge = &machine.blocks[3];
    assert_eq!(
        merge.parameters.iter().map(|p| p.id).collect::<Vec<_>>(),
        vec![value(42)],
        "the v41 copy collapses and the dead v43 drops"
    );
    let Terminator::Jump { arguments, .. } = &machine.blocks[1].terminator else {
        panic!("b2 keeps its jump")
    };
    assert_eq!(arguments, &[value(20)]);
    assert_eq!(
        machine.blocks[0].operations.len()
            + machine.blocks[1].operations.len()
            + machine.blocks[2].operations.len(),
        2,
        "only the two live constant producers remain"
    );
}
