//! Catalog-driven post-allocation symbolic-machine optimization stage.
//!
//! [`catalog`] is the single enable/disable and ordering point. Target leaves
//! own custody joins for their exact independently validated symbolic plans.

mod aarch64_cbnz;
mod aarch64_movn;
pub mod catalog;
mod execution;
mod model;
mod x86_xor_zero;

pub use aarch64_cbnz::*;
pub use aarch64_movn::*;
pub use catalog::{ORDERED_RULES, PostAllocationMachineCatalogError, require_rule, selected_rule};
pub use execution::*;
pub use model::*;
pub use x86_xor_zero::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedPostAllocationMachineOptimizationError {
    Source(crate::OptimizedPostAllocationMachinePipelineError),
    MissingPostAllocationMachineOptimization,
    UnsupportedPostAllocationMachineOptimization(omega_optimization_core::Optimization),
    Fusion(omega_machine_optimizer::Aarch64CbnzFusionError),
    MovnMaterialization(omega_machine_optimizer::Aarch64MovnMaterializationError),
    X86XorZeroMaterialization(omega_machine_optimizer::X86XorZeroMaterializationError),
    ReceiptMismatch,
}

impl From<PostAllocationMachineCatalogError> for OptimizedPostAllocationMachineOptimizationError {
    fn from(error: PostAllocationMachineCatalogError) -> Self {
        match error {
            PostAllocationMachineCatalogError::MissingSelection => {
                Self::MissingPostAllocationMachineOptimization
            }
            PostAllocationMachineCatalogError::UnsupportedSelection(optimization)
            | PostAllocationMachineCatalogError::UnsupportedComposition(optimization) => {
                Self::UnsupportedPostAllocationMachineOptimization(optimization)
            }
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
