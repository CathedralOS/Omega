//! Optimizer module role: executable entrance. Catalog-driven post-allocation rule execution.
//!
//! One catalog dispatch operates on current allocation facts, independent of
//! which selected-program rewrite produced them. Validation is a separate seam.

mod dispatch;
mod validation;

use omega_machine_optimizer::selected_post_allocation_machine_rule;

use super::{
    OptimizedPostAllocationMachineOptimizationError,
    StagedOptimizedPostAllocationMachineOptimization,
};
use crate::StagedOptimizedPostAllocationMachinePlan;

pub use dispatch::stage_optimized_post_allocation_machine_optimization_for_catalog_entry;
pub use validation::validate_optimized_post_allocation_machine_optimization_custody;

pub fn stage_optimized_post_allocation_machine_optimization(
    source: &impl crate::AllocationSource,
    machine: &StagedOptimizedPostAllocationMachinePlan,
) -> Result<
    StagedOptimizedPostAllocationMachineOptimization,
    OptimizedPostAllocationMachineOptimizationError,
> {
    let allocation = crate::replay_machine_source(source, machine)?;
    let phase = allocation
        .selections()
        .project_phase(omega_optimization_core::OptimizationExecutionPhase::PostAllocationMachine);
    let entry = selected_post_allocation_machine_rule(
        &phase,
        machine.machine().plan().target.architecture,
    )?
    .0;
    stage_optimized_post_allocation_machine_optimization_for_catalog_entry(source, machine, entry)
}
