//! Exact fixed-view-copy and active-resident-rematerialization routes.

use crate::tests::{
    AdmissionProfile, ExplicitOptimizationRequest, FixedViewCopyPolicy,
    FunctionRelativeOptimizationUnavailableData, NativeTarget, Optimization,
    OptimizationSelections, PostAllocationSelectedTransformation,
    StagedAllocationRecoveryFunctionRelativeSource, StagedOptimizedVerifiedPhysicalPipeline,
    conditional_active_resident_exact_add_chain_artifact, conditional_forwarded_parameter_artifact,
    optimize_artifact_sections, selected_lowering_budget,
    stage_optimized_verified_physical_pipeline_with_provider_executions,
    validate_optimized_post_allocation_machine_plan_after_fixed_view_copy_custody,
};

#[test]
fn compiler_facing_physical_pipeline_runs_only_the_named_shared_entry_copy() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (semantic, proof) = conditional_forwarded_parameter_artifact();
        let selections = OptimizationSelections::new([
            Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1,
        ])
        .unwrap();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            ExplicitOptimizationRequest::new(selections.clone(), selected_lowering_budget())
                .unwrap(),
        )
        .unwrap();
        let staged = stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized,
            target,
            &[],
        )
        .unwrap();
        let StagedOptimizedVerifiedPhysicalPipeline::AllocationRecovery { realization } = &staged
        else {
            panic!("the exact allocation-recovery phase must use its fixed-copy route")
        };
        let StagedAllocationRecoveryFunctionRelativeSource::FixedViewCopies(homes) =
            realization.source()
        else {
            panic!("the fixed-view rule must retain fixed-view source custody")
        };
        let machine = realization.machine();
        let reanalysis = homes.reanalysis_stage();
        let copies = reanalysis.transformation_stage();
        let plan = copies.copies().plan();
        assert_eq!(staged.selections(), selections.identity());
        assert_eq!(staged.selected_lowering_completion(), None);
        assert_eq!(staged.function_relative_manifest(), realization.manifest());
        assert!(staged.post_allocation_machine_optimization().is_none());
        assert_eq!(
            copies.custody().policy(),
            FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1
        );
        assert_eq!(copies.custody().copy_count(), 1);
        assert_eq!(plan.copies.len(), 1);
        assert_eq!(plan.copies[0].destinations.len(), 2);
        assert_eq!(reanalysis.custody().entry_transition_count(), 0);
        assert_eq!(reanalysis.legality().receipt().entry_transition_count(), 0);
        assert_eq!(
            machine.machine().receipt().post_allocation_manifest(),
            homes.post_allocation_manifest().record().identity
        );
        assert_eq!(
            &validate_optimized_post_allocation_machine_plan_after_fixed_view_copy_custody(
                homes, machine,
            )
            .unwrap(),
            machine.custody()
        );
    }
}

#[test]
fn compiler_facing_physical_pipeline_runs_only_the_named_active_resident_rematerialization() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (semantic, proof) = conditional_active_resident_exact_add_chain_artifact();
        let selections = OptimizationSelections::new([
            Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
        ])
        .unwrap();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            ExplicitOptimizationRequest::new(selections.clone(), selected_lowering_budget())
                .unwrap(),
        )
        .unwrap();
        let staged = stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized,
            target,
            &[],
        )
        .unwrap();
        let StagedOptimizedVerifiedPhysicalPipeline::AllocationRecovery { realization } = &staged
        else {
            panic!("the exact rematerialization selection must use its owning realization")
        };
        let StagedAllocationRecoveryFunctionRelativeSource::ActiveResidentRematerialization(
            rematerialization,
        ) = realization.source()
        else {
            panic!("the active-resident rule must retain rematerialization source custody")
        };
        let manifest = realization.manifest().record();
        let empty = OptimizationSelections::default().identity();
        assert_eq!(staged.selections(), selections.identity());
        assert_eq!(staged.selected_lowering_completion(), None);
        assert!(
            staged
                .selected_lowering_function_relative_realization()
                .is_none()
        );
        assert_eq!(
            staged
                .allocation_recovery_function_relative_realization()
                .unwrap()
                .custody(),
            realization.custody()
        );
        assert_eq!(staged.function_relative_manifest(), realization.manifest());
        assert!(staged.post_allocation_machine_optimization().is_none());
        assert_eq!(
            manifest.allocation_recovery_selections,
            selections.identity()
        );
        assert_eq!(manifest.selected_lowering_selections, empty);
        assert_eq!(manifest.post_allocation_machine_selections, empty);
        assert_eq!(manifest.function_relative_layout_selections, empty);
        assert_eq!(manifest.selected_lowering_completion, None);
        assert_eq!(rematerialization.custody().applied_count(), 1);
        assert_eq!(rematerialization.custody().rewritten_use_count(), 2);
        assert_eq!(
            staged
                .post_allocation_manifest()
                .record()
                .selected_transformations,
            [
                PostAllocationSelectedTransformation::PressureRematerialization(
                    rematerialization.rematerialization().receipt().identity(),
                )
            ]
        );
        assert_eq!(
            staged.machine().machine().receipt().selected(),
            manifest.selected
        );
        assert_eq!(
            manifest.publication,
            FunctionRelativeOptimizationUnavailableData::Unavailable
        );
    }
}
