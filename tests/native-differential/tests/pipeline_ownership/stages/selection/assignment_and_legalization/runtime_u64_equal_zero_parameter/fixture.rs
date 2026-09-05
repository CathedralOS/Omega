use crate::tests::*;

pub(super) fn staged_equal_zero_parameter(
    target: NativeTarget,
) -> StagedOptimizedSelectedInstructions {
    let (semantic, proof) = conditional_u64_equal_zero_parameter_artifact();
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
    let target = lower_optimized_to_target_operations(optimized, target).unwrap();
    stage_optimized_instruction_selection(target).unwrap()
}
