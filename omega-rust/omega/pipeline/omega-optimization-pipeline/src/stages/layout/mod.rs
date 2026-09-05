//! Optimizer module role: stage group. Resolved selected-form layout, relaxation, and exit-contract stages.

#[cfg(test)]
pub(crate) use omega_machine_emission::Rel8ExitBoundaryForTest;
pub use omega_machine_emission::{
    ValidatedWholeFunctionExitContract, WholeFunctionEntryAssumption, WholeFunctionExitContract,
    WholeFunctionExitContractError, WholeFunctionExitContractIdentity, WholeFunctionExitEvidence,
    WholeFunctionExitLayoutCustody, WholeFunctionExitPolicy, WholeFunctionFrameDisposition,
    WholeFunctionHardeningPolicy, WholeFunctionReturnEvidence, WholeFunctionReturnMechanism,
    WholeFunctionReturnValueEvidence, WholeFunctionStructuralUnitCallEvidence,
    WholeFunctionStructuralUnitExitEvidence, stage_whole_function_exit_contract,
    stage_whole_function_exit_contract_after_aarch64_cbnz_fusion,
    stage_whole_function_exit_contract_after_x86_branch_relaxation,
    stage_whole_function_exit_contract_with_frame,
    stage_whole_function_exit_contract_with_post_allocation_machine_optimization,
    validate_whole_function_exit_contract,
    validate_whole_function_exit_contract_after_aarch64_cbnz_fusion,
    validate_whole_function_exit_contract_after_x86_branch_relaxation,
    validate_whole_function_exit_contract_with_frame,
    validate_whole_function_exit_contract_with_post_allocation_machine_optimization,
};
pub use omega_selected_form_encoding_to_resolved_layout::*;
