//! Reject retired machine-rewrite custody at the common exit boundary.
use super::super::{error::WholeFunctionExitContractError, model::WholeFunctionExitLayoutCustody};
use machine_code::ResolvedMachineLayout;
use post_allocation_machine_to_selected_form_encoding::StagedOptimizedSelectedFormEncoding;
use register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan;

pub(in crate::exit_contract) fn validate_layout_custody(
    _machine: &StagedOptimizedPostAllocationMachinePlan,
    encoding: &StagedOptimizedSelectedFormEncoding,
    layout: &ResolvedMachineLayout,
    custody: WholeFunctionExitLayoutCustody,
) -> Result<(), WholeFunctionExitContractError> {
    if !matches!(
        custody,
        WholeFunctionExitLayoutCustody::BaselineNearLayoutV1
            | WholeFunctionExitLayoutCustody::X86RelaxConditionalBranchesToRel8V1 { .. }
    ) || encoding.post_allocation_machine_optimization().is_some()
        || layout.post_allocation_machine_optimization().is_some()
    {
        return Err(WholeFunctionExitContractError::OptimizationCustodyMismatch);
    }
    Ok(())
}
