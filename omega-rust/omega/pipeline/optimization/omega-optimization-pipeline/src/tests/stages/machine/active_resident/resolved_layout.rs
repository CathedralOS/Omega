//! Resolved layout projection and mutation rejection.

use crate::tests::{
    NativeTarget, OptimizedActiveResidentRematerializationResolvedSelectedFormLayoutError,
    OptimizedActiveResidentRematerializationSelectedFormEncodingError,
    OptimizedResolvedSelectedFormLayoutError, OptimizedSelectedFormEncodingError,
    SelectedFunctionLayoutPolicy,
    stage_optimized_active_resident_rematerialization_resolved_selected_form_layout,
    stage_optimized_active_resident_rematerialization_selected_form_encoding,
    staged_active_resident_rematerialization_and_machine, staged_active_resident_resolved_layout,
    validate_optimized_active_resident_rematerialization_resolved_selected_form_layout,
};

#[test]
fn active_resident_rematerialization_reaches_resolved_layout_on_both_architectures() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (source, machine) = staged_active_resident_rematerialization_and_machine(target);
        let fresh_materialize = source.rematerialization().plan().functions[0]
            .action
            .as_ref()
            .unwrap()
            .fresh_materialize;
        let physical = source
            .source()
            .live_range_stage()
            .liveness_stage()
            .selected_stage()
            .register_environment()
            .physical()
            .identity();
        let pre_layout = stage_optimized_active_resident_rematerialization_selected_form_encoding(
            source, machine,
        )
        .unwrap();
        let pre_layout_custody = pre_layout.custody().clone();
        let selected = pre_layout.encoding().selected();
        let machine = pre_layout.encoding().machine();
        let pre_layout_encoding = pre_layout.encoding().identity();

        let staged =
            stage_optimized_active_resident_rematerialization_resolved_selected_form_layout(
                pre_layout,
            )
            .unwrap();
        let layout = staged.layout();
        let custody = staged.custody();
        assert_eq!(custody.pre_layout_custody(), &pre_layout_custody);
        assert_eq!(custody.selected(), selected);
        assert_eq!(custody.machine(), machine);
        assert_eq!(custody.pre_layout(), pre_layout_encoding);
        assert_eq!(custody.physical(), physical);
        assert_eq!(custody.layout(), layout.identity());
        assert_eq!(custody.target(), target);
        assert_eq!(
            custody.policy(),
            SelectedFunctionLayoutPolicy::EntryThenZeroFallthroughThenNonzeroV1
        );
        assert_eq!(custody.function_count(), 1);
        assert_eq!(custody.block_count(), 3);
        assert_eq!(
            custody.instruction_count(),
            custody.pre_layout_custody().row_count()
        );
        assert_eq!(
            custody.instruction_count(),
            layout
                .functions()
                .iter()
                .flat_map(|function| &function.blocks)
                .map(|block| block.instructions.len())
                .sum::<usize>()
        );
        assert_eq!(
            custody.byte_count(),
            layout
                .functions()
                .iter()
                .map(|function| function.byte_count)
                .sum::<u64>()
        );
        assert_eq!(custody.resolved_branch_count(), 1);
        let rows = layout
            .functions()
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .collect::<Vec<_>>();
        let fresh_row = rows
            .iter()
            .find(|row| row.instruction == fresh_materialize)
            .expect("fresh rematerialization must survive resolved layout");
        assert_eq!(
            fresh_row.alternative.family,
            omega_selected_instructions::MachineAlternativeFamily::MaterializeI64
        );
        assert!(!fresh_row.bytes.is_empty());
        assert_eq!(
            rows.iter().filter(|row| row.branch.is_some()).count(),
            custody.resolved_branch_count()
        );
        assert_eq!(
            validate_optimized_active_resident_rematerialization_resolved_selected_form_layout(
                &staged,
            )
            .unwrap(),
            custody.clone()
        );
    }
}

#[test]
fn active_resident_resolved_layout_rejects_pre_layout_layout_and_receipt_mutation() {
    let mut corrupt_pre_layout = staged_active_resident_resolved_layout(NativeTarget::linux_x64());
    crate::stages::layout::active_resident_resolved_selected_form_layout::corrupt_active_resident_resolved_layout_pre_layout_byte_for_test(
        &mut corrupt_pre_layout,
    );
    assert_eq!(
        validate_optimized_active_resident_rematerialization_resolved_selected_form_layout(
            &corrupt_pre_layout,
        ),
        Err(
            OptimizedActiveResidentRematerializationResolvedSelectedFormLayoutError::PreLayout(
                OptimizedActiveResidentRematerializationSelectedFormEncodingError::Encoding(
                    OptimizedSelectedFormEncodingError::ArtifactMismatch,
                ),
            ),
        )
    );

    let mut corrupt_layout = staged_active_resident_resolved_layout(NativeTarget::linux_x64());
    crate::stages::layout::active_resident_resolved_selected_form_layout::corrupt_active_resident_resolved_layout_byte_for_test(
        &mut corrupt_layout,
    );
    assert_eq!(
        validate_optimized_active_resident_rematerialization_resolved_selected_form_layout(
            &corrupt_layout,
        ),
        Err(
            OptimizedActiveResidentRematerializationResolvedSelectedFormLayoutError::Layout(
                OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch,
            ),
        )
    );

    let mut corrupt_receipt = staged_active_resident_resolved_layout(NativeTarget::linux_x64());
    crate::stages::layout::active_resident_resolved_selected_form_layout::corrupt_active_resident_resolved_layout_receipt_for_test(
        &mut corrupt_receipt,
    );
    assert_eq!(
        validate_optimized_active_resident_rematerialization_resolved_selected_form_layout(
            &corrupt_receipt,
        ),
        Err(
            OptimizedActiveResidentRematerializationResolvedSelectedFormLayoutError::ReceiptMismatch,
        )
    );
}
