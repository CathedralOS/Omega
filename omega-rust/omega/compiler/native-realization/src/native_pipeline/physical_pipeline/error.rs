use abstract_operations_to_target_operations::LoweringError;

use machine_emission::{
    AllocationRecoveryFunctionRelativeRealizationError,
    FunctionRelativeOptimizationRealizationError,
    OptimizedStructuralUnitFunctionRelativeRealizationError,
    OptimizedUnitFunctionRelativeRealizationError,
};
use post_allocation_machine_to_post_allocation_machine::OptimizedPostAllocationMachineOptimizationError;
use register_homes_to_post_allocation_machine::OptimizedPostAllocationMachinePipelineError;
use target_operations_to_selected_instructions::OptimizedSelectionPipelineError;

#[derive(Debug)]
pub enum OptimizedVerifiedPhysicalPipelineError {
    SelectedOptimization(
        selected_instructions_to_selected_instructions::SelectedInstructionOptimizationError,
    ),
    PostTerminalSelectionMismatch,
    UnconsumedPostTerminalPhase(optimization_core::OptimizationExecutionPhase),
    TargetLowering(LoweringError),
    RegisterEnvironment(register_environment::TargetRegisterEnvironmentValidationError),
    Selection(OptimizedSelectionPipelineError),
    RegisterAllocation(selected_instructions_to_register_homes::RegisterAllocationError),
    PostAllocationMachine(OptimizedPostAllocationMachinePipelineError),
    PostAllocationMachineOptimization(OptimizedPostAllocationMachineOptimizationError),
    PostAllocationMachineRuleCatalog(
        post_allocation_machine_to_post_allocation_machine::PostAllocationMachineRuleCatalogError,
    ),
    SelectedLoweringRuleCatalog(
        selected_instructions_to_register_homes::SelectedLoweringRuleCatalogError,
    ),
    AllocationRecoveryRuleCatalog(
        selected_instructions_to_register_homes::AllocationRecoveryRuleCatalogError,
    ),
    FunctionRelativeLayoutRuleCatalog(
        resolved_layout_to_resolved_layout::FunctionRelativeLayoutCatalogError,
    ),
    AllocationRecoveryFunctionRelative(Box<AllocationRecoveryFunctionRelativeRealizationError>),
    UnitFunctionRelativeRealization(OptimizedUnitFunctionRelativeRealizationError),
    StructuralUnitFunctionRelativeRealization(
        OptimizedStructuralUnitFunctionRelativeRealizationError,
    ),
    UnsupportedPhysicalPhaseComposition,
    FunctionRelativeRealization(FunctionRelativeOptimizationRealizationError),
}

impl std::fmt::Display for OptimizedVerifiedPhysicalPipelineError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized verified physical staging failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedVerifiedPhysicalPipelineError {}
