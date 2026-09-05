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
        omega_selected_instructions_to_register_homes::AllocatedCalleeSavedRequirementError,
    ),
    CalleeSaveStorage(
        omega_post_allocation_machine_to_frame_layout::NonAuthoritativeCalleeSaveStorageError,
    ),
    FrameLayout(omega_post_allocation_machine_to_frame_layout::TargetFrameLayoutError),
    FrameProtocol(crate::TargetFrameProtocolEncodingError),
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
