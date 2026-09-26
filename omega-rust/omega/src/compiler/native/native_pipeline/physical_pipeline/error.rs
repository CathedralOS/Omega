use abstract_operations_to_target_operations::LoweringError;
use register_homes_to_post_allocation_machine::OptimizedPostAllocationMachinePipelineError;
use resolved_layout_to_resolved_layout::machine_emission::FunctionRelativeOptimizationRealizationError;
use target_operations_to_selected_instructions::OptimizedSelectionPipelineError;

#[derive(Debug)]
pub enum OptimizedVerifiedPhysicalPipelineError {
    SelectedOptimization(
        selected_instructions_to_selected_instructions::SelectedInstructionOptimizationError,
    ),
    PostTerminalSelectionMismatch,
    UnconsumedPostTerminalPhase(optimization_core::OptimizationExecutionPhase),
    TargetLowering(LoweringError),
    RegisterEnvironment(target_operations_to_selected_instructions::register_environment::TargetRegisterEnvironmentValidationError),
    Selection(OptimizedSelectionPipelineError),
    RegisterAllocation(selected_instructions_to_register_homes::RegisterAllocationError),
    PostAllocationMachine(OptimizedPostAllocationMachinePipelineError),
    AllocationRecoveryRuleCatalog(
        selected_instructions_to_selected_instructions::AllocationRecoveryRuleCatalogError,
    ),
    SelectedLoweringRuleCatalog(
        selected_instructions_to_selected_instructions::SelectedLoweringRuleCatalogError,
    ),
    PreAllocationRuleCatalog(
        selected_instructions_to_selected_instructions::PreAllocationRuleCatalogError,
    ),
    FunctionRelativeLayoutRuleCatalog(
        resolved_layout_to_resolved_layout::FunctionRelativeLayoutCatalogError,
    ),
    FunctionRelativeRealization(FunctionRelativeOptimizationRealizationError),
}

impl std::fmt::Display for OptimizedVerifiedPhysicalPipelineError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "common physical staging failed: {self:?}")
    }
}

impl std::error::Error for OptimizedVerifiedPhysicalPipelineError {}
