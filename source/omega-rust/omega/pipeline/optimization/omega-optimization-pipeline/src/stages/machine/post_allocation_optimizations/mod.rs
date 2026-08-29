//! Catalog-driven post-allocation symbolic-machine optimization stage.
//!
//! `omega_machine_optimizer::rules` owns the single enable/order catalog.
//! [`execution`] consumes that catalog, while target leaves here retain
//! pipeline custody for their independently validated symbolic plans.

mod aarch64_cbnz;
mod aarch64_movn;
mod execution;
mod model;
mod x86_xor_zero;

pub use aarch64_cbnz::*;
pub use aarch64_movn::*;
pub use execution::*;
pub use model::*;
pub use x86_xor_zero::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedPostAllocationMachineOptimizationError {
    Source(crate::OptimizedPostAllocationMachinePipelineError),
    MissingPostAllocationMachineOptimization,
    UnsupportedPostAllocationMachineOptimization(omega_optimization_core::Optimization),
    UnsupportedPostAllocationMachineOptimizationTarget {
        optimization: omega_optimization_core::Optimization,
        required: omega_target::Architecture,
        actual: omega_target::Architecture,
    },
    Fusion(omega_machine_optimizer::Aarch64CbnzFusionError),
    MovnMaterialization(omega_machine_optimizer::Aarch64MovnMaterializationError),
    X86XorZeroMaterialization(omega_machine_optimizer::X86XorZeroMaterializationError),
    ReceiptMismatch,
}

impl From<omega_machine_optimizer::PostAllocationMachineRuleCatalogError>
    for OptimizedPostAllocationMachineOptimizationError
{
    fn from(error: omega_machine_optimizer::PostAllocationMachineRuleCatalogError) -> Self {
        match error {
            omega_machine_optimizer::PostAllocationMachineRuleCatalogError::MissingSelection => {
                Self::MissingPostAllocationMachineOptimization
            }
            omega_machine_optimizer::PostAllocationMachineRuleCatalogError::UnsupportedSelection(
                optimization,
            )
            | omega_machine_optimizer::PostAllocationMachineRuleCatalogError::UnsupportedComposition(
                optimization,
            ) => {
                Self::UnsupportedPostAllocationMachineOptimization(optimization)
            }
            omega_machine_optimizer::PostAllocationMachineRuleCatalogError::UnsupportedTarget {
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
