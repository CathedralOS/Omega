//! Independent replay comparison and constraint receipt sealing.

use crate::{
    AbstractSpillAccessConstraintError, AbstractSpillAccessConstraintPlan,
    AbstractSpillAccessConstraintReceipt, AbstractSpillAccessDependencyReason,
    ValidatedAbstractSpillAccessConstraints, ValidatedAbstractSpillMemoryEffects,
    abstract_spill_access_constraint_plan_identity,
};

pub fn validate_abstract_spill_access_constraints(
    source: &ValidatedAbstractSpillMemoryEffects,
    candidate: AbstractSpillAccessConstraintPlan,
) -> Result<ValidatedAbstractSpillAccessConstraints, AbstractSpillAccessConstraintError> {
    let roots = source.receipt();
    if candidate.abstract_spill_memory_effects != roots.identity()
        || candidate.register_environment != roots.register_environment()
        || candidate.allocator_availability != roots.allocator_availability()
        || candidate.optimization_unit != roots.optimization_unit()
        || candidate.fuel_schedule != roots.fuel_schedule()
    {
        return Err(AbstractSpillAccessConstraintError::RootMismatch);
    }
    let expected = super::replay::replay(source, candidate.policy, candidate.budget)?;
    if candidate.usage != expected.usage {
        return Err(AbstractSpillAccessConstraintError::UsageMismatch);
    }
    if candidate.functions != expected.functions {
        return Err(AbstractSpillAccessConstraintError::NonCanonicalFunctions);
    }
    let placement_count = candidate
        .functions
        .iter()
        .map(|row| row.placements.len())
        .sum();
    let dependency_count = candidate
        .functions
        .iter()
        .map(|row| row.dependencies.len())
        .sum();
    let count_reason = |reason: fn(&AbstractSpillAccessDependencyReason) -> bool| {
        candidate
            .functions
            .iter()
            .flat_map(|row| &row.dependencies)
            .filter(|dependency| reason(&dependency.reason))
            .count()
    };
    let max_spill_area_bytes = candidate
        .functions
        .iter()
        .map(|row| row.spill_area_bytes)
        .max()
        .unwrap_or(0);
    let receipt = AbstractSpillAccessConstraintReceipt {
        identity: abstract_spill_access_constraint_plan_identity(&candidate),
        abstract_spill_memory_effects: candidate.abstract_spill_memory_effects,
        register_environment: candidate.register_environment,
        allocator_availability: candidate.allocator_availability,
        optimization_unit: candidate.optimization_unit,
        fuel_schedule: candidate.fuel_schedule,
        usage: candidate.usage,
        function_count: candidate.functions.len(),
        placement_count,
        dependency_count,
        stored_value_dependency_count: count_reason(|reason| {
            matches!(
                reason,
                AbstractSpillAccessDependencyReason::StoredValue { .. }
            )
        }),
        declared_barrier_count: count_reason(|reason| {
            matches!(
                reason,
                AbstractSpillAccessDependencyReason::DeclaredBeforeReload
            )
        }),
        overlapping_slice_dependency_count: count_reason(|reason| {
            matches!(
                reason,
                AbstractSpillAccessDependencyReason::OverlappingAbstractSlice { .. }
            )
        }),
        max_spill_area_bytes,
    };
    Ok(ValidatedAbstractSpillAccessConstraints {
        plan: candidate,
        receipt,
    })
}
