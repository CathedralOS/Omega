use super::StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayout;

pub(crate) fn corrupt_active_resident_resolved_layout_pre_layout_byte_for_test(
    staged: &mut StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayout,
) {
    crate::corrupt_active_resident_selected_form_encoding_byte_for_test(&mut staged.pre_layout);
}

pub(crate) fn corrupt_active_resident_resolved_layout_byte_for_test(
    staged: &mut StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayout,
) {
    let byte = staged
        .layout
        .functions_mut()
        .iter_mut()
        .flat_map(|function| &mut function.blocks)
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| instruction.bytes.first_mut())
        .expect("active-resident resolved-layout fixture must contain encoded bytes");
    *byte ^= 1;
}

pub(crate) fn corrupt_active_resident_resolved_layout_receipt_for_test(
    staged: &mut StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayout,
) {
    staged.custody.resolved_branch_count ^= 1;
}
