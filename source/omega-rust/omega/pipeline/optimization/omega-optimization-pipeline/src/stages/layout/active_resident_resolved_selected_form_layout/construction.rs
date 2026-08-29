use crate::{
    SelectedFunctionLayoutPolicy,
    StagedOptimizedActiveResidentRematerializationSelectedFormEncoding,
    StagedOptimizedActiveResidentRematerializationSelectedFormEncodingCustodyReceipt,
    StagedOptimizedResolvedSelectedFormLayout, stage_optimized_resolved_selected_form_layout,
    validate_optimized_active_resident_rematerialization_selected_form_encoding,
};

use super::OptimizedActiveResidentRematerializationResolvedSelectedFormLayoutError;

pub(super) fn construct_active_resident_resolved_selected_form_layout(
    pre_layout: &StagedOptimizedActiveResidentRematerializationSelectedFormEncoding,
) -> Result<
    (
        StagedOptimizedActiveResidentRematerializationSelectedFormEncodingCustodyReceipt,
        omega_register_model::PhysicalRegisterModelIdentity,
        StagedOptimizedResolvedSelectedFormLayout,
    ),
    OptimizedActiveResidentRematerializationResolvedSelectedFormLayoutError,
> {
    let pre_layout_custody =
        validate_optimized_active_resident_rematerialization_selected_form_encoding(pre_layout)
            .map_err(
                OptimizedActiveResidentRematerializationResolvedSelectedFormLayoutError::PreLayout,
            )?;
    let selected = pre_layout.source().rematerialization();
    let machine = pre_layout.machine();
    let physical = pre_layout
        .source()
        .source()
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .register_environment()
        .physical();
    let layout = stage_optimized_resolved_selected_form_layout(
        selected,
        machine,
        physical,
        pre_layout.encoding(),
    )
    .map_err(OptimizedActiveResidentRematerializationResolvedSelectedFormLayoutError::Layout)?;
    if layout.policy() != SelectedFunctionLayoutPolicy::EntryThenZeroFallthroughThenNonzeroV1 {
        return Err(
            OptimizedActiveResidentRematerializationResolvedSelectedFormLayoutError::ReceiptMismatch,
        );
    }
    Ok((pre_layout_custody, physical.identity(), layout))
}
