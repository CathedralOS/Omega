use omega_abstract_operations_to_target_operations::LoweringError;

use crate::{
    AllocationRecoveryFunctionRelativeRealizationError,
    FunctionRelativeOptimizationRealizationError, OptimizedActiveResidentRematerializationError,
    OptimizedAllocationLegalityCustodyError, OptimizedFixedViewCopyCustodyError,
    OptimizedLiteralFoldCustodyError, OptimizedLiveRangeCustodyError,
    OptimizedLivenessCustodyError, OptimizedPostAllocationMachineOptimizationError,
    OptimizedPostAllocationMachinePipelineError, OptimizedPostCopyRegisterHomeCustodyError,
    OptimizedPostSelectedLoweringHomeCustodyError, OptimizedRegisterHomeCustodyError,
    OptimizedSelectedReanalysisError, OptimizedSelectionPipelineError,
};

#[derive(Debug)]
pub enum OptimizedVerifiedPhysicalPipelineError {
    TargetLowering(LoweringError),
    Selection(OptimizedSelectionPipelineError),
    Liveness(OptimizedLivenessCustodyError),
    LiveRanges(OptimizedLiveRangeCustodyError),
    AllocationLegality(OptimizedAllocationLegalityCustodyError),
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
