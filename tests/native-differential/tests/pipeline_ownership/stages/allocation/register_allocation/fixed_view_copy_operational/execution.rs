//! Exact-selection disablement through the public physical pipeline.

use crate::tests::{
    AdmissionProfile, ExplicitOptimizationRequest, Optimization, OptimizationSelections,
    OptimizedRegisterHomeCustodyError, OptimizedVerifiedPhysicalPipelineError, RegisterHomeError,
    conditional_forwarded_parameter_artifact, optimize_artifact_sections, selected_lowering_budget,
    stage_optimized_verified_physical_pipeline_with_provider_executions,
};

use super::fixture::targets;

#[test]
fn shared_entry_fixed_view_copy_is_disabled_without_its_exact_selection() {
    for target in targets() {
        let (semantic, proof) = conditional_forwarded_parameter_artifact();
        let selections = OptimizationSelections::new([Optimization::CopyPropagation]).unwrap();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            ExplicitOptimizationRequest::new(selections.clone(), selected_lowering_budget())
                .unwrap(),
        )
        .unwrap();
        let error = stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized,
            target,
            &[],
        )
        .unwrap_err();

        // The forwarded parameter deliberately requires this exact recovery.
        // Selecting an unrelated Psi rule must not silently enable the copy or
        // fall back to an unrequested physical transformation.
        assert!(matches!(
            error,
            OptimizedVerifiedPhysicalPipelineError::RegisterAllocation(
                selected_instructions_to_register_homes::RegisterAllocationError::Homes(
                    OptimizedRegisterHomeCustodyError::Assignment(
                        RegisterHomeError::UnresolvedEntryTransitions {
                            function: 0,
                            register: 1,
                            count: 2,
                        },
                    ),
                ),
            )
        ));
    }
}
