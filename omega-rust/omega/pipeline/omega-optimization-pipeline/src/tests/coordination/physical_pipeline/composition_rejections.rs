//! Fail-closed rejection of unsupported physical-phase compositions.

use crate::tests::{
    AdmissionProfile, ExplicitOptimizationRequest, IntegerValue, NativeTarget, Optimization,
    OptimizationSelections, OptimizedVerifiedPhysicalPipelineError,
    conditional_active_resident_exact_add_chain_artifact,
    conditional_active_resident_exact_add_chain_artifact_with_false_literal,
    optimize_artifact_sections, selected_lowering_budget,
    stage_optimized_verified_physical_pipeline_with_provider_executions,
};

#[test]
fn allocation_recovery_compositions_reject_instead_of_dispatching_a_hidden_policy() {
    let (semantic, proof) = conditional_active_resident_exact_add_chain_artifact();
    for (selections, same_phase) in [
        (
            OptimizationSelections::new([
                Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1,
                Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
            ])
            .unwrap(),
            true,
        ),
        (
            OptimizationSelections::new([
                Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
                Optimization::SelectedIncomingU12ExactAddImmediate,
            ])
            .unwrap(),
            false,
        ),
    ] {
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            ExplicitOptimizationRequest::new(selections, selected_lowering_budget()).unwrap(),
        )
        .unwrap();
        let result = stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized,
            NativeTarget::linux_x64(),
            &[],
        );
        if same_phase {
            assert!(matches!(
                result,
                Err(
                    OptimizedVerifiedPhysicalPipelineError::AllocationRecoveryRuleCatalog(
                        omega_selected_instructions_to_register_homes::AllocationRecoveryRuleCatalogError::UnsupportedComposition
                    )
                )
            ));
        } else {
            assert!(matches!(
                result,
                Err(OptimizedVerifiedPhysicalPipelineError::UnsupportedPhysicalPhaseComposition)
            ));
        }
    }
}

#[test]
fn unadmitted_fixed_view_copy_machine_pair_rejects_without_fallback() {
    let (semantic, proof) = conditional_active_resident_exact_add_chain_artifact();
    for (machine_optimization, target) in [
        (
            Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1,
            NativeTarget::linux_x64(),
        ),
        (
            Optimization::Aarch64ElideSameViewCopyI64BeforeReturnV1,
            NativeTarget::linux_arm64(),
        ),
    ] {
        let selections = OptimizationSelections::new([
            Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1,
            machine_optimization,
        ])
        .unwrap();
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
                target,
                &[],
            ),
            Err(OptimizedVerifiedPhysicalPipelineError::UnsupportedPhysicalPhaseComposition)
        ));
    }
}

#[test]
fn aarch64_post_allocation_machine_composition_rejects_without_hidden_ordering_policy() {
    let (semantic, proof) = conditional_active_resident_exact_add_chain_artifact_with_false_literal(
        IntegerValue::Unsigned(u64::MAX as u128),
    );
    let selections = OptimizationSelections::new([
        Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1,
        Optimization::Aarch64SelectShortestMovnSeededI64MaterializationV1,
    ])
    .unwrap();
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
            OptimizedVerifiedPhysicalPipelineError::PostAllocationMachineRuleCatalog(
                omega_machine_optimizer::PostAllocationMachineRuleCatalogError::UnsupportedComposition(
                    Optimization::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1
                )
            )
        )
    ));
}
