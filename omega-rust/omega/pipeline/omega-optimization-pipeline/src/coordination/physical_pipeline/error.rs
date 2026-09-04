use omega_abstract_operations_to_target_operations::LoweringError;

use crate::{
    AllocationRecoveryFunctionRelativeRealizationError,
    FunctionRelativeOptimizationRealizationError, OptimizedActiveResidentRematerializationError,
    OptimizedAllocationLegalityCustodyError, OptimizedFixedPrecoloredSegmentHomeCustodyError,
    OptimizedFixedViewCopyCustodyError, OptimizedLiteralFoldCustodyError,
    OptimizedLiveRangeCustodyError, OptimizedLivenessCustodyError,
    OptimizedPostAllocationMachineOptimizationError, OptimizedPostAllocationMachinePipelineError,
    OptimizedPostCopyRegisterHomeCustodyError, OptimizedPostSelectedLoweringHomeCustodyError,
    OptimizedRegisterHomeCustodyError, OptimizedSelectedReanalysisError,
    OptimizedSelectionPipelineError, OptimizedStructuralUnitFunctionRelativeRealizationError,
    OptimizedUnitFunctionRelativeRealizationError,
};

#[derive(Debug)]
pub enum OptimizedVerifiedPhysicalPipelineError {
    PostTerminalSelectionMismatch,
    UnconsumedPostTerminalPhase(omega_optimization_core::OptimizationExecutionPhase),
    TargetLowering(LoweringError),
    Selection(OptimizedSelectionPipelineError),
    Liveness(OptimizedLivenessCustodyError),
    LiveRanges(OptimizedLiveRangeCustodyError),
    AllocationLegality(OptimizedAllocationLegalityCustodyError),
    FixedPrecoloredSegmentHomes(OptimizedFixedPrecoloredSegmentHomeCustodyError),
    RegisterHomes(OptimizedRegisterHomeCustodyError),
    SelectedLowering(OptimizedLiteralFoldCustodyError),
    SelectedLoweringHomes(OptimizedPostSelectedLoweringHomeCustodyError),
    PostAllocationMachine(OptimizedPostAllocationMachinePipelineError),
    PostAllocationMachineOptimization(OptimizedPostAllocationMachineOptimizationError),
    PostAllocationMachineRuleCatalog(
        omega_machine_optimizer::PostAllocationMachineRuleCatalogError,
    ),
    SelectedLoweringRuleCatalog(omega_regalloc::SelectedLoweringRuleCatalogError),
    AllocationRecoveryRuleCatalog(omega_regalloc::AllocationRecoveryRuleCatalogError),
    FunctionRelativeLayoutRuleCatalog(crate::FunctionRelativeLayoutCatalogError),
    FixedViewCopies(OptimizedFixedViewCopyCustodyError),
    SelectedReanalysis(OptimizedSelectedReanalysisError),
    PostCopyRegisterHomes(OptimizedPostCopyRegisterHomeCustodyError),
    ActiveResidentRematerialization(OptimizedActiveResidentRematerializationError),
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
