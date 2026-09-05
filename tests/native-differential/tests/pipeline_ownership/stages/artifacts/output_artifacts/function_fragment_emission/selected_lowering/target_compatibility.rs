//! Fail-closed target compatibility for an architecture-specific layout selection.

use crate::tests::*;

#[test]
fn x86_rel8_selection_rejects_a_non_x86_target_without_a_realization() {
    let (semantic, proof) = conditional_exact_binary_artifact(false);
    let selections =
        OptimizationSelections::new([Optimization::X86RelaxConditionalBranchesToRel8V1]).unwrap();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(selections, selected_lowering_budget()).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized,
            NativeTarget::linux_arm64(),
            &[],
        ),
        Err(
            OptimizedVerifiedPhysicalPipelineError::FunctionRelativeLayoutRuleCatalog(
                FunctionRelativeLayoutCatalogError::UnsupportedTarget {
                    optimization: Optimization::X86RelaxConditionalBranchesToRel8V1,
                    required: target::Architecture::X86_64,
                    actual: target::Architecture::Aarch64,
                }
            )
        )
    ));
}
