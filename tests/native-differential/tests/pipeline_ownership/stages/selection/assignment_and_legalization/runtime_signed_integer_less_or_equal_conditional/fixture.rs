use crate::tests::{
    AdmissionProfile, NativeTarget, Optimization, OptimizationSelections,
    OptimizedTargetLoweringRequest, StagedOptimizedSelectedInstructions,
    conditional_i64_integer_less_or_equal_parameters_artifact,
    lower_optimized_to_target_operations, optimize_artifact_sections, request,
    stage_optimized_instruction_selection,
};
pub(super) fn staged_signed_integer_less_or_equal_conditional(
    target: NativeTarget,
) -> StagedOptimizedSelectedInstructions {
    let (semantic, proof) = conditional_i64_integer_less_or_equal_parameters_artifact();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
    )
    .unwrap();
    let target = lower_optimized_to_target_operations(
        optimized,
        OptimizedTargetLoweringRequest::new(target),
    )
    .unwrap();
    stage_optimized_instruction_selection(target).unwrap()
}
