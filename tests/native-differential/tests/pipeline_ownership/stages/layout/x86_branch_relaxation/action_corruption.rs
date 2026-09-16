//! Public-validator rejection of independently reauthenticated evidence mutations:
//! action bytes, the recorded attempt roster, and drift in retained-layout rows
//! the rewrite never touched.

use crate::tests::{
    FunctionRelativeOptimizationRealizationError, OptimizedX86BranchRelaxationError,
    ResolvedLayoutOptimizationError, validate_fixed_frame_function_relative_realization,
};

fn expect_artifact_mismatch(
    realization: &crate::tests::StagedFixedFrameFunctionRelativeRealization,
) {
    assert_eq!(
        validate_fixed_frame_function_relative_realization(realization),
        Err(
            FunctionRelativeOptimizationRealizationError::LayoutOptimization(
                ResolvedLayoutOptimizationError::Relaxation(
                    OptimizedX86BranchRelaxationError::ArtifactMismatch,
                ),
            ),
        ),
    );
}

#[test]
fn authenticated_action_corruption_rejects_at_the_public_realization_boundary() {
    let mut realization = super::fixture::direct_realization();
    realization
        .relaxation_mut()
        .unwrap()
        .corrupt_first_action_bytes_and_reauthenticate_for_test();

    expect_artifact_mismatch(&realization);
}

#[test]
fn authenticated_attempt_roster_corruption_rejects_at_the_public_realization_boundary() {
    let mut realization = super::fixture::direct_realization();
    realization
        .relaxation_mut()
        .unwrap()
        .corrupt_first_attempt_outcome_and_reauthenticate_for_test();

    expect_artifact_mismatch(&realization);
}

#[test]
fn authenticated_retained_layout_drift_rejects_at_the_public_realization_boundary() {
    let mut realization = super::fixture::direct_realization();
    realization
        .relaxation_mut()
        .unwrap()
        .corrupt_retained_layout_row_and_reauthenticate_for_test();

    expect_artifact_mismatch(&realization);
}
