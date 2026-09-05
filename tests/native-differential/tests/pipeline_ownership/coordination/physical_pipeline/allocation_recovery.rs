//! Exact fixed-view-copy and active-resident-rematerialization routes.

use crate::tests::{
    AdmissionProfile, AllocationEvidence, ExplicitOptimizationRequest, FixedViewCopyPolicy,
    FunctionRelativeOptimizationUnavailableData, NativeTarget, Optimization,
    OptimizationSelections, PostAllocationSelectedTransformation,
    conditional_active_resident_exact_add_chain_artifact, conditional_forwarded_parameter_artifact,
    optimize_artifact_sections, selected_lowering_budget,
    stage_optimized_verified_physical_pipeline_with_provider_executions,
    validate_optimized_post_allocation_machine_plan_custody,
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
        let realization = (staged).allocation_recovery_for_test().unwrap_or_else(|| {
            panic!("the exact allocation-recovery phase must use its fixed-copy route")
        });
        let current = realization.allocation().current();
        let copies = realization
            .allocation()
            .fixed_view_copy_proof_for_test()
            .unwrap();
        let AllocationEvidence::FixedViewCopies(home_receipt) = current.evidence() else {
            panic!("fixture must retain fixed-view evidence")
        };
        let machine = realization.machine();
        let reanalysis = home_receipt.source();
        let copy_receipt = reanalysis.source();
        let plan = copies.plan();
        assert_eq!(staged.selections(), selections.identity());
        assert_eq!(staged.selected_lowering_completion(), None);
        assert_eq!(staged.function_relative_manifest(), realization.manifest());
        assert!(staged.post_allocation_machine_optimization().is_none());
        assert_eq!(
            copy_receipt.policy(),
            FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1
        );
        assert_eq!(copy_receipt.copy_count(), 1);
        assert_eq!(plan.copies.len(), 1);
        assert_eq!(plan.copies[0].destinations.len(), 2);
        assert_eq!(reanalysis.entry_transition_count(), 0);
        assert_eq!(current.legality().receipt().entry_transition_count(), 0);
        assert_eq!(
            machine.machine().receipt().post_allocation_manifest(),
            current.post_allocation_manifest().record().identity
        );
        assert_eq!(
            &validate_optimized_post_allocation_machine_plan_custody(&current, machine,).unwrap(),
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
        let realization = (staged).allocation_recovery_for_test().unwrap_or_else(|| {
            panic!("the exact rematerialization selection must use its owning realization")
        });
        let current = realization.allocation().current();
        let rematerialization = realization
            .allocation()
            .rematerialization_proof_for_test()
            .unwrap();
        let AllocationEvidence::ActiveResidentRematerialization(recovery_receipt) =
            current.evidence()
        else {
            panic!("fixture must retain rematerialization evidence")
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
        assert_eq!((*recovery_receipt).applied_count(), 1);
        assert_eq!((*recovery_receipt).rewritten_use_count(), 2);
        assert_eq!(
            staged
                .post_allocation_manifest()
                .record()
                .selected_transformations,
            [
                PostAllocationSelectedTransformation::PressureRematerialization(
                    rematerialization.receipt().identity(),
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
