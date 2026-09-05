use omega_abstract_operations_to_target_operations::LoweringError;

use omega_machine_emission::{
    AllocationRecoveryFunctionRelativeRealizationError,
    FunctionRelativeOptimizationRealizationError,
    OptimizedStructuralUnitFunctionRelativeRealizationError,
    OptimizedUnitFunctionRelativeRealizationError,
};
use omega_post_allocation_machine_to_optimized_machine::OptimizedPostAllocationMachineOptimizationError;
use omega_register_homes_to_post_allocation_machine::OptimizedPostAllocationMachinePipelineError;
use omega_selected_instructions_to_register_homes::{
    OptimizedAllocationLegalityCustodyError, OptimizedLiteralFoldCustodyError,
    OptimizedLiveRangeCustodyError, OptimizedLivenessCustodyError,
    OptimizedPostSelectedLoweringHomeCustodyError, OptimizedRegisterHomeCustodyError,
};
use omega_target_operations_to_selected_instructions::OptimizedSelectionPipelineError;

#[derive(Debug)]
pub enum OptimizedVerifiedPhysicalPipelineError {
    PostTerminalSelectionMismatch,
    UnconsumedPostTerminalPhase(omega_optimization_core::OptimizationExecutionPhase),
    TargetLowering(LoweringError),
    RegisterEnvironment(
        omega_target_to_register_environment::TargetRegisterEnvironmentValidationError,
    ),
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
    FunctionRelativeLayoutRuleCatalog(
        omega_selected_form_encoding_to_resolved_layout::FunctionRelativeLayoutCatalogError,
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
