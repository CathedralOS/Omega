//! Public-validator rejection of an independently reauthenticated action mutation.

use crate::tests::*;

#[test]
fn authenticated_action_corruption_rejects_at_the_public_realization_boundary() {
    let mut realization = super::fixture::direct_realization();
    realization
        .relaxation_mut()
        .corrupt_first_action_bytes_and_reauthenticate_for_test();

    assert_eq!(
        validate_function_relative_layout_optimization_realization_custody(&realization),
        Err(
            FunctionRelativeOptimizationRealizationError::LayoutOptimization(
                ResolvedLayoutOptimizationError::Relaxation(
                    OptimizedX86BranchRelaxationError::ArtifactMismatch,
                ),
            ),
        ),
    );
}
