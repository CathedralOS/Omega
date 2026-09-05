use omega_abstract_operations_to_target_operations::LoweringError;

use crate::{
    AllocationRecoveryFunctionRelativeRealizationError,
    FunctionRelativeOptimizationRealizationError, OptimizedAllocationLegalityCustodyError,
    OptimizedLiteralFoldCustodyError, OptimizedLiveRangeCustodyError,
    OptimizedLivenessCustodyError, OptimizedPostAllocationMachineOptimizationError,
    OptimizedPostAllocationMachinePipelineError, OptimizedPostSelectedLoweringHomeCustodyError,
    OptimizedRegisterHomeCustodyError, OptimizedSelectionPipelineError,
    OptimizedStructuralUnitFunctionRelativeRealizationError,
    OptimizedUnitFunctionRelativeRealizationError,
};

#[derive(Debug)]
pub enum OptimizedVerifiedPhysicalPipelineError {
    PostTerminalSelectionMismatch,
    UnconsumedPostTerminalPhase(omega_optimization_core::OptimizationExecutionPhase),
    TargetLowering(LoweringError),
    RegisterEnvironment(crate::TargetRegisterEnvironmentValidationError),
    Selection(OptimizedSelectionPipelineError),
    Liveness(OptimizedLivenessCustodyError),
    LiveRanges(OptimizedLiveRangeCustodyError),
    AllocationLegality(OptimizedAllocationLegalityCustodyError),
    RegisterAllocation(omega_selected_instructions_to_register_homes::RegisterAllocationError),
    AllocationReplay(omega_selected_instructions_to_register_homes::AllocationReplayError),
    RegisterHomes(OptimizedRegisterHomeCustodyError),
    SelectedLowering(OptimizedLiteralFoldCustodyError),
    SelectedLoweringHomes(OptimizedPostSelectedLoweringHomeCustodyError),
    PostAllocationMachine(OptimizedPostAllocationMachinePipelineError),
    PostAllocationMachineOptimization(OptimizedPostAllocationMachineOptimizationError),
    PostAllocationMachineRuleCatalog(
        omega_post_allocation_machine_to_optimized_machine::PostAllocationMachineRuleCatalogError,
    ),
    SelectedLoweringRuleCatalog(
        omega_selected_instructions_to_register_homes::SelectedLoweringRuleCatalogError,
    ),
    AllocationRecoveryRuleCatalog(
        omega_selected_instructions_to_register_homes::AllocationRecoveryRuleCatalogError,
    ),
    FunctionRelativeLayoutRuleCatalog(crate::FunctionRelativeLayoutCatalogError),
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
