use crate::{
    StagedOptimizedActiveResidentRematerialization,
    StagedOptimizedActiveResidentRematerializationCustodyReceipt,
    StagedOptimizedPostAllocationMachineCustodyReceipt, StagedOptimizedPostAllocationMachinePlan,
    StagedOptimizedSelectedFormEncoding, stage_optimized_layout_independent_selected_form_encoding,
    validate_optimized_active_resident_rematerialization,
    validate_optimized_post_allocation_machine_plan_custody,
};

use super::OptimizedActiveResidentRematerializationSelectedFormEncodingError;

pub(super) fn construct_active_resident_selected_form_encoding(
    source: &StagedOptimizedActiveResidentRematerialization,
    machine: &StagedOptimizedPostAllocationMachinePlan,
) -> Result<
    (
        StagedOptimizedActiveResidentRematerializationCustodyReceipt,
        StagedOptimizedPostAllocationMachineCustodyReceipt,
        StagedOptimizedSelectedFormEncoding,
    ),
    OptimizedActiveResidentRematerializationSelectedFormEncodingError,
> {
    let rematerialization = validate_optimized_active_resident_rematerialization(source).map_err(
        OptimizedActiveResidentRematerializationSelectedFormEncodingError::Rematerialization,
    )?;
    let machine_custody = validate_optimized_post_allocation_machine_plan_custody(source, machine)
        .map_err(OptimizedActiveResidentRematerializationSelectedFormEncodingError::Machine)?;
    let environment = source
        .source()
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .register_environment();
    let encoding = stage_optimized_layout_independent_selected_form_encoding(
        source.rematerialization(),
        machine,
        environment.physical(),
    )
    .map_err(OptimizedActiveResidentRematerializationSelectedFormEncodingError::Encoding)?;
    Ok((rematerialization, machine_custody, encoding))
}
