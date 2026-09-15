use crate::tests::{
    AdmissionProfile, ExplicitOptimizationRequest, NativeTarget, Optimization,
    OptimizationSelections, OptimizedTargetLoweringRequest, StagedOptimizedSelectedInstructions,
    conditional_u64_not_equal_zero_parameter_artifact, lower_optimized_to_target_operations,
    optimize_artifact_sections, selected_lowering_budget, stage_optimized_instruction_selection,
};
pub(super) fn staged_not_equal_zero_parameter(
    target: NativeTarget,
) -> StagedOptimizedSelectedInstructions {
    let (semantic, proof) = conditional_u64_not_equal_zero_parameter_artifact();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        ExplicitOptimizationRequest::new(
            OptimizationSelections::new([Optimization::CopyPropagation]).unwrap(),
            selected_lowering_budget(),
        )
        .unwrap(),
    )
    .unwrap();
    let target = lower_optimized_to_target_operations(
        optimized,
        OptimizedTargetLoweringRequest::new(target),
    )
    .unwrap();
    stage_optimized_instruction_selection(target).unwrap()
}
