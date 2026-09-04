#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Catalog-driven post-allocation optimization components.
//!
//! `omega_machine_optimizer::rules` owns the single enable/order catalog.
//! [`execution`] is the executable catalog consumer, while target leaves retain
//! pipeline custody for their independently validated symbolic plans.

mod aarch64_cbnz;
mod aarch64_movn;
mod aarch64_same_view_copy;
mod execution;
mod model;
mod x86_mov_r32_imm32;
mod x86_mov_r64_imm32_sign_extended;
mod x86_xor_zero;

pub use aarch64_cbnz::*;
pub use aarch64_movn::*;
pub use aarch64_same_view_copy::*;
pub use execution::*;
pub use model::*;
pub use x86_mov_r32_imm32::*;
pub use x86_mov_r64_imm32_sign_extended::*;
pub use x86_xor_zero::*;

use omega_allocation_legality_to_active_resident_rematerialization::StagedOptimizedActiveResidentRematerialization;
use omega_allocation_legality_to_register_homes::StagedOptimizedRegisterHomes;
use omega_literal_folds_to_register_homes::StagedOptimizedRegisterHomesAfterSelectedLowering;
use omega_register_homes_to_post_allocation_machine::{
    OptimizedPostAllocationMachinePipelineError, StagedOptimizedPostAllocationMachinePlan,
    validate_optimized_post_allocation_machine_plan_after_active_resident_rematerialization_custody,
    validate_optimized_post_allocation_machine_plan_after_selected_lowering_custody,
    validate_optimized_post_allocation_machine_plan_custody,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedPostAllocationMachineOptimizationError {
    Source(OptimizedPostAllocationMachinePipelineError),
    MissingPostAllocationMachineOptimization,
    UnsupportedPostAllocationMachineOptimization(omega_optimization_core::Optimization),
    UnsupportedPostAllocationMachineOptimizationTarget {
        optimization: omega_optimization_core::Optimization,
        required: omega_target::Architecture,
        actual: omega_target::Architecture,
    },
    Fusion(omega_machine_optimizer::Aarch64CbnzFusionError),
    MovnMaterialization(omega_machine_optimizer::Aarch64MovnMaterializationError),
    SameViewCopyElision(omega_machine_optimizer::Aarch64SameViewCopyElisionError),
    X86XorZeroMaterialization(omega_machine_optimizer::X86XorZeroMaterializationError),
    X86MovR32Imm32Materialization(omega_machine_optimizer::X86MovR32Imm32MaterializationError),
    X86MovR64Imm32SignExtendedMaterialization(
        omega_machine_optimizer::X86MovR64Imm32SignExtendedMaterializationError,
    ),
    SelectionProjectionMismatch,
    ReceiptMismatch,
}

impl From<omega_machine_optimizer::PostAllocationMachineRuleCatalogError>
    for OptimizedPostAllocationMachineOptimizationError
{
    fn from(error: omega_machine_optimizer::PostAllocationMachineRuleCatalogError) -> Self {
        match error {
            omega_machine_optimizer::PostAllocationMachineRuleCatalogError::WrongPhase(_) => {
                Self::SelectionProjectionMismatch
            }
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
