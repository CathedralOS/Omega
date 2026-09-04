use crate::tests::*;

#[test]
fn unit_legalization_retains_a_static_attachment_without_inventing_a_receiver() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (semantic, proof, attachment) = statically_attached_unit_return_artifact();
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            request(OptimizationSelections::new([Optimization::CopyPropagation]).unwrap()),
        )
        .unwrap();
        let target = lower_optimized_to_target_operations(optimized, target).unwrap();
        let selected = stage_optimized_instruction_selection(target).unwrap();

        assert_eq!(
            selected.selected().plan().functions[0].attachment,
            Some(attachment)
        );
        assert!(
            selected.selected().plan().functions[0]
                .virtual_registers
                .is_empty()
        );
    }
}
