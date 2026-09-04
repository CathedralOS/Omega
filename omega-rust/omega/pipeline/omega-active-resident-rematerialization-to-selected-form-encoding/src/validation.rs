use crate::{
    validate_optimized_active_resident_rematerialization,
    validate_optimized_layout_independent_selected_form_encoding,
    validate_optimized_post_allocation_machine_plan_custody,
};

use super::{
    OptimizedActiveResidentRematerializationSelectedFormEncodingError,
    StagedOptimizedActiveResidentRematerializationSelectedFormEncoding,
    StagedOptimizedActiveResidentRematerializationSelectedFormEncodingCustodyReceipt,
    custody::project_active_resident_selected_form_encoding_custody,
};

pub fn validate_optimized_active_resident_rematerialization_selected_form_encoding(
    staged: &StagedOptimizedActiveResidentRematerializationSelectedFormEncoding,
) -> Result<
    StagedOptimizedActiveResidentRematerializationSelectedFormEncodingCustodyReceipt,
    OptimizedActiveResidentRematerializationSelectedFormEncodingError,
> {
    let rematerialization = validate_optimized_active_resident_rematerialization(&staged.source)
        .map_err(
            OptimizedActiveResidentRematerializationSelectedFormEncodingError::Rematerialization,
        )?;
    let machine =
        validate_optimized_post_allocation_machine_plan_custody(&staged.source, &staged.machine)
            .map_err(OptimizedActiveResidentRematerializationSelectedFormEncodingError::Machine)?;
    let environment = staged
        .source
        .source()
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .register_environment();
    validate_optimized_layout_independent_selected_form_encoding(
        staged.source.rematerialization(),
        &staged.machine,
        environment.physical(),
        &staged.encoding,
    )
    .map_err(OptimizedActiveResidentRematerializationSelectedFormEncodingError::Encoding)?;
    let custody = project_active_resident_selected_form_encoding_custody(
        rematerialization,
        machine,
        &staged.encoding,
    );
    if custody != staged.custody {
        return Err(
            OptimizedActiveResidentRematerializationSelectedFormEncodingError::ReceiptMismatch,
        );
    }
    Ok(custody)
}
