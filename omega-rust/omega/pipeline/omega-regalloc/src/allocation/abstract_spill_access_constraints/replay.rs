//! Independently keyed reconstruction of abstract access constraints.

mod accesses;
mod dependencies;
mod work;

use omega_optimization_core::OptimizationWorkBudget;

use crate::{
    AbstractSpillAccessConstraintError, AbstractSpillAccessConstraintPlan,
    AbstractSpillAccessConstraintPolicy, FunctionAbstractSpillAccessConstraints,
    ValidatedAbstractSpillMemoryEffects,
};

pub(super) fn replay(
    source: &ValidatedAbstractSpillMemoryEffects,
    policy: AbstractSpillAccessConstraintPolicy,
    budget: OptimizationWorkBudget,
) -> Result<AbstractSpillAccessConstraintPlan, AbstractSpillAccessConstraintError> {
    if !matches!(
        policy,
        AbstractSpillAccessConstraintPolicy::BlockLocalDataBarrierAndOverlapV1
    ) {
        return Err(AbstractSpillAccessConstraintError::UnsupportedPolicy);
    }
    let mut functions = Vec::new();
    for (function, row) in source.plan().functions.iter().enumerate() {
        let placements = accesses::reconstruct(function, row)?;
        let dependencies = dependencies::reconstruct(function, row, &placements)?;
        functions.push(FunctionAbstractSpillAccessConstraints {
            machine: row.machine,
            spill_area_bytes: row.spill_area_bytes,
            placements,
            dependencies,
        });
    }
    let usage = work::reconstruct(&functions)?;
    if !usage.within(budget) {
        return Err(AbstractSpillAccessConstraintError::BudgetExceeded {
            required: usage,
            budget,
        });
    }
    let roots = source.receipt();
    Ok(AbstractSpillAccessConstraintPlan {
        abstract_spill_memory_effects: roots.identity(),
        register_environment: roots.register_environment(),
        allocator_availability: roots.allocator_availability(),
        optimization_unit: roots.optimization_unit(),
        fuel_schedule: roots.fuel_schedule(),
        policy,
        budget,
        usage,
        functions,
    })
}
