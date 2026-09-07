use crate::tests::*;

#[test]
fn structural_unit_return_reaches_legalization_and_stops_before_selection() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (semantic, proof) = structurally_parameterized_unit_return_artifact();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
        )
        .unwrap();
        let target = lower_optimized_to_target_operations(optimized, target).unwrap();

        let result = stage_optimized_instruction_selection(target);
        assert!(
            matches!(
                result,
                Err(OptimizedSelectionPipelineError::Selection(
                    SelectedInstructionError::UnsupportedSourceShape { function: 0 }
                ))
            ),
            "unexpected selection result: {result:?}"
        );
    }
}
