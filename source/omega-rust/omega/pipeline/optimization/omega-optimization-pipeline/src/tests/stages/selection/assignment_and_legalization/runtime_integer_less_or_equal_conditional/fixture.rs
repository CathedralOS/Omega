use crate::tests::*;

pub(super) fn staged_integer_less_or_equal_conditional(
    target: NativeTarget,
) -> StagedOptimizedSelectedInstructions {
    let (semantic, proof) = conditional_u64_integer_less_or_equal_parameters_artifact();
    let optimized = optimize_artifact_sections(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
    )
    .unwrap();
    let target = lower_optimized_to_target_operations(optimized, target).unwrap();
    stage_optimized_instruction_selection(target).unwrap()
}
