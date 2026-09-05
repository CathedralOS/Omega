use abstract_operations_to_target_operations::LoweringError;

use machine_emission::{
    AllocationRecoveryFunctionRelativeRealizationError,
    FunctionRelativeOptimizationRealizationError,
    OptimizedStructuralUnitFunctionRelativeRealizationError,
    OptimizedUnitFunctionRelativeRealizationError,
};
use post_allocation_machine_to_post_allocation_machine::OptimizedPostAllocationMachineOptimizationError;
use register_homes_to_post_allocation_machine::OptimizedPostAllocationMachinePipelineError;
use selected_instructions_to_register_homes::{
    OptimizedAllocationLegalityCustodyError, OptimizedLiteralFoldCustodyError,
    OptimizedLiveRangeCustodyError, OptimizedLivenessCustodyError,
    OptimizedPostSelectedLoweringHomeCustodyError, OptimizedRegisterHomeCustodyError,
};
use target_operations_to_selected_instructions::OptimizedSelectionPipelineError;

#[derive(Debug)]
pub enum OptimizedVerifiedPhysicalPipelineError {
    PostTerminalSelectionMismatch,
    UnconsumedPostTerminalPhase(optimization_core::OptimizationExecutionPhase),
    TargetLowering(LoweringError),
    RegisterEnvironment(target_to_register_environment::TargetRegisterEnvironmentValidationError),
    Selection(OptimizedSelectionPipelineError),
    Liveness(OptimizedLivenessCustodyError),
    LiveRanges(OptimizedLiveRangeCustodyError),
    AllocationLegality(OptimizedAllocationLegalityCustodyError),
    RegisterAllocation(selected_instructions_to_register_homes::RegisterAllocationError),
    AllocationReplay(selected_instructions_to_register_homes::AllocationReplayError),
    RegisterHomes(OptimizedRegisterHomeCustodyError),
    SelectedLowering(OptimizedLiteralFoldCustodyError),
    SelectedLoweringHomes(OptimizedPostSelectedLoweringHomeCustodyError),
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
        selected_form_encoding_to_resolved_layout::FunctionRelativeLayoutCatalogError,
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
