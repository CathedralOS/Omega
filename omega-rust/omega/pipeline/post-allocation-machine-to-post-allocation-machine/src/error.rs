//! Failures in machine-rule selection, replay and source custody.
use register_homes_to_post_allocation_machine::OptimizedPostAllocationMachinePipelineError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedPostAllocationMachineOptimizationError {
    Source(OptimizedPostAllocationMachinePipelineError),
    MissingPostAllocationMachineOptimization,
    UnsupportedPostAllocationMachineOptimization(optimization_core::Optimization),
    UnsupportedPostAllocationMachineOptimizationTarget {
        optimization: optimization_core::Optimization,
        required: target::Architecture,
        actual: target::Architecture,
    },
    Fusion(crate::Aarch64CbnzFusionError),
    MovnMaterialization(crate::Aarch64MovnMaterializationError),
    SameViewCopyElision(crate::Aarch64SameViewCopyElisionError),
    X86XorZeroMaterialization(crate::X86XorZeroMaterializationError),
    X86MovR32Imm32Materialization(crate::X86MovR32Imm32MaterializationError),
    X86MovR64Imm32SignExtendedMaterialization(
        crate::X86MovR64Imm32SignExtendedMaterializationError,
    ),
    SelectionProjectionMismatch,
    ReceiptMismatch,
}

impl From<crate::PostAllocationMachineRuleCatalogError>
    for OptimizedPostAllocationMachineOptimizationError
{
    fn from(error: crate::PostAllocationMachineRuleCatalogError) -> Self {
        match error {
            crate::PostAllocationMachineRuleCatalogError::WrongPhase(_) => {
                Self::SelectionProjectionMismatch
            }
            crate::PostAllocationMachineRuleCatalogError::MissingSelection => {
                Self::MissingPostAllocationMachineOptimization
            }
            crate::PostAllocationMachineRuleCatalogError::UnsupportedSelection(optimization)
            | crate::PostAllocationMachineRuleCatalogError::UnsupportedComposition(optimization) => {
                Self::UnsupportedPostAllocationMachineOptimization(optimization)
            }
            crate::PostAllocationMachineRuleCatalogError::UnsupportedTarget {
                optimization,
                required,
                actual,
            } => Self::UnsupportedPostAllocationMachineOptimizationTarget {
                optimization,
                required,
                actual,
            },
        }
    }
}

impl std::fmt::Display for OptimizedPostAllocationMachineOptimizationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized post-allocation machine transformation failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedPostAllocationMachineOptimizationError {}
