use super::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FunctionRelativeOptimizationRealizationError {
    Allocation(omega_selected_instructions_to_register_homes::AllocationReplayError),
    PostAllocationMachine(OptimizedPostAllocationMachinePipelineError),
    PostAllocationMachineOptimization(OptimizedPostAllocationMachineOptimizationError),
    Encoding(OptimizedSelectedFormEncodingError),
    Layout(OptimizedResolvedSelectedFormLayoutError),
    X86BranchRelaxation(OptimizedX86BranchRelaxationError),
    RuleCatalog(
        omega_selected_form_encoding_to_resolved_layout::FunctionRelativeLayoutCatalogError,
    ),
    ExitContract(WholeFunctionExitContractError),
    CalleeSavedRequirements(
        omega_register_homes_to_callee_saved_requirements::AllocatedCalleeSavedRequirementError,
    ),
    CalleeSaveStorage(
        omega_callee_saved_requirements_to_save_storage::NonAuthoritativeCalleeSaveStorageError,
    ),
    FrameLayout(omega_post_allocation_machine_to_frame_layout::TargetFrameLayoutError),
    FrameProtocol(omega_frame_layout_to_frame_protocol::TargetFrameProtocolEncodingError),
    MissingFunctionRelativeLayoutOptimization,
    OptimizationCustodyUnavailable,
    StatisticsOverflow,
    RootMismatch,
    ReceiptMismatch,
}

impl std::fmt::Display for FunctionRelativeOptimizationRealizationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "function-relative optimization realization failed: {self:?}"
        )
    }
}

impl std::error::Error for FunctionRelativeOptimizationRealizationError {}
