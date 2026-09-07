//! Exact-selection disablement through the public physical pipeline.

use crate::tests::{
    AdmissionProfile, ExplicitOptimizationRequest, Optimization, OptimizationSelections,
    conditional_forwarded_parameter_artifact, optimize_artifact_sections, selected_lowering_budget,
    stage_optimized_verified_physical_pipeline_with_provider_executions,
};

use super::fixture::targets;

#[test]
fn ordinary_abi_transfers_publish_without_enabling_optional_fixed_view_copy() {
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
        let pipeline = stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized,
            target,
            &[],
        )
        .unwrap();
        // Entry/return transfers are ordinary selected instructions, so this
        // program needs no optional fixed-view-copy recovery.
        assert!(
            pipeline
                .post_allocation_manifest()
                .record()
                .selected_transformations
                .is_empty()
        );
        assert_eq!(
            pipeline
                .post_allocation_manifest()
                .record()
                .statistics
                .fixed_view_transitions,
            0
        );
    }
}
