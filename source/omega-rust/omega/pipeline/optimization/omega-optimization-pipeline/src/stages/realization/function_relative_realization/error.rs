use super::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FunctionRelativeOptimizationRealizationError {
    Homes(OptimizedPostSelectedLoweringHomeCustodyError),
    DirectHomes(OptimizedRegisterHomeCustodyError),
    PostAllocationMachine(OptimizedPostAllocationMachinePipelineError),
    PostAllocationMachineOptimization(OptimizedPostAllocationMachineOptimizationError),
    Encoding(OptimizedSelectedFormEncodingError),
    Layout(OptimizedResolvedSelectedFormLayoutError),
    X86BranchRelaxation(OptimizedX86BranchRelaxationError),
    RuleCatalog(crate::FunctionRelativeLayoutCatalogError),
    ExitContract(WholeFunctionExitContractError),
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
