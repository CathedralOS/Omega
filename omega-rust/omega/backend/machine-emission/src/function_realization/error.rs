use crate::exit_contract::WholeFunctionExitContractError;
use post_allocation_machine_to_selected_form_encoding::OptimizedSelectedFormEncodingError;
use register_homes_to_post_allocation_machine::OptimizedPostAllocationMachinePipelineError;
use resolved_layout_to_resolved_layout::ResolvedLayoutOptimizationError;
use selected_form_encoding_to_resolved_layout::OptimizedResolvedSelectedFormLayoutError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FunctionRelativeOptimizationRealizationError {
    Allocation(selected_instructions_to_register_homes::AllocationReplayError),
    PostAllocationMachine(OptimizedPostAllocationMachinePipelineError),
    Encoding(OptimizedSelectedFormEncodingError),
    Layout(OptimizedResolvedSelectedFormLayoutError),
    LayoutOptimization(ResolvedLayoutOptimizationError),
    ExitContract(WholeFunctionExitContractError),
    CalleeSavedRequirements(
        selected_instructions_to_register_homes::AllocatedCalleeSavedRequirementError,
    ),
    CalleeSaveStorage(crate::frame_layout::NonAuthoritativeCalleeSaveStorageError),
    FrameLayout(crate::frame_layout::TargetFrameLayoutError),
    FrameProtocol(crate::TargetFrameProtocolEncodingError),
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
