use crate::{
    SelectedFunctionLayoutPolicy,
    validate_optimized_active_resident_rematerialization_selected_form_encoding,
    validate_optimized_resolved_selected_form_layout,
};

use super::{
    OptimizedActiveResidentRematerializationResolvedSelectedFormLayoutError,
    StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayout,
    StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayoutCustodyReceipt,
    custody::project_active_resident_resolved_layout_custody,
};

pub fn validate_optimized_active_resident_rematerialization_resolved_selected_form_layout(
    staged: &StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayout,
) -> Result<
    StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayoutCustodyReceipt,
    OptimizedActiveResidentRematerializationResolvedSelectedFormLayoutError,
> {
    let pre_layout_custody =
        validate_optimized_active_resident_rematerialization_selected_form_encoding(
            &staged.pre_layout,
        )
        .map_err(
            OptimizedActiveResidentRematerializationResolvedSelectedFormLayoutError::PreLayout,
        )?;
    let selected = staged.pre_layout.source().rematerialization();
    let machine = staged.pre_layout.machine();
    let physical = staged
        .pre_layout
        .source()
        .source()
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .register_environment()
        .physical();
    validate_optimized_resolved_selected_form_layout(
        selected,
        machine,
        physical,
        staged.pre_layout.encoding(),
        &staged.layout,
    )
    .map_err(OptimizedActiveResidentRematerializationResolvedSelectedFormLayoutError::Layout)?;
    if staged.layout.policy() != SelectedFunctionLayoutPolicy::EntryThenZeroFallthroughThenNonzeroV1
    {
        return Err(
            OptimizedActiveResidentRematerializationResolvedSelectedFormLayoutError::ReceiptMismatch,
        );
    }
    let custody = project_active_resident_resolved_layout_custody(
        pre_layout_custody,
        physical.identity(),
        &staged.layout,
    );
    if custody != staged.custody {
        return Err(
            OptimizedActiveResidentRematerializationResolvedSelectedFormLayoutError::ReceiptMismatch,
        );
    }
    Ok(custody)
}
