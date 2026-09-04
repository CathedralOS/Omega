//! Optimizer module role: executable entrance. Catalog-driven post-allocation rule execution.
//!
//! Both retained source lineages resolve exactly one catalog row here before
//! typed rule dispatch. Independent custody validation remains a public seam.

mod dispatch;
mod source_lineage;
mod validation;

use omega_machine_optimizer::selected_post_allocation_machine_rule;

use super::{
    OptimizedPostAllocationMachineOptimizationError,
    StagedOptimizedPostAllocationMachineOptimization,
};
use crate::{
    StagedOptimizedActiveResidentRematerialization, StagedOptimizedPostAllocationMachinePlan,
    StagedOptimizedRegisterHomes, StagedOptimizedRegisterHomesAfterSelectedLowering,
};

pub use dispatch::{
    stage_optimized_post_allocation_machine_optimization_after_active_resident_rematerialization_for_catalog_entry,
    stage_optimized_post_allocation_machine_optimization_after_selected_lowering_for_catalog_entry,
    stage_optimized_post_allocation_machine_optimization_for_catalog_entry,
};
pub use validation::{
    validate_optimized_post_allocation_machine_optimization_after_active_resident_rematerialization_custody,
    validate_optimized_post_allocation_machine_optimization_after_selected_lowering_custody,
    validate_optimized_post_allocation_machine_optimization_custody,
};

pub fn stage_optimized_post_allocation_machine_optimization_after_active_resident_rematerialization(
    source: &StagedOptimizedActiveResidentRematerialization,
    machine: &StagedOptimizedPostAllocationMachinePlan,
) -> Result<
    StagedOptimizedPostAllocationMachineOptimization,
    OptimizedPostAllocationMachineOptimizationError,
> {
    let phase = source_lineage::active_resident_selections(source)
        .project_phase(omega_optimization_core::OptimizationExecutionPhase::PostAllocationMachine);
    let entry = selected_post_allocation_machine_rule(
        &phase,
        machine.machine().plan().target.architecture,
    )?
    .0;
    stage_optimized_post_allocation_machine_optimization_after_active_resident_rematerialization_for_catalog_entry(
        source, machine, entry,
    )
}

pub fn stage_optimized_post_allocation_machine_optimization(
    source: &StagedOptimizedRegisterHomes,
    machine: &StagedOptimizedPostAllocationMachinePlan,
) -> Result<
    StagedOptimizedPostAllocationMachineOptimization,
    OptimizedPostAllocationMachineOptimizationError,
> {
    let phase = source_lineage::register_home_selections(source)
        .project_phase(omega_optimization_core::OptimizationExecutionPhase::PostAllocationMachine);
    let entry = selected_post_allocation_machine_rule(
        &phase,
        machine.machine().plan().target.architecture,
    )?
    .0;
    stage_optimized_post_allocation_machine_optimization_for_catalog_entry(source, machine, entry)
}

pub fn stage_optimized_post_allocation_machine_optimization_after_selected_lowering(
    source: &StagedOptimizedRegisterHomesAfterSelectedLowering,
    machine: &StagedOptimizedPostAllocationMachinePlan,
) -> Result<
    StagedOptimizedPostAllocationMachineOptimization,
    OptimizedPostAllocationMachineOptimizationError,
> {
    let phase = source_lineage::selected_lowering_selections(source)
        .project_phase(omega_optimization_core::OptimizationExecutionPhase::PostAllocationMachine);
    let entry = selected_post_allocation_machine_rule(
        &phase,
        machine.machine().plan().target.architecture,
    )?
    .0;
    stage_optimized_post_allocation_machine_optimization_after_selected_lowering_for_catalog_entry(
        source, machine, entry,
    )
}
