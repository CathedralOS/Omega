//! Independent replay comparison and fixed/precolored receipt sealing.

use crate::{
    FixedPrecoloredIntervalError, FixedPrecoloredIntervalPlan,
    FixedPrecoloredIntervalValidationReceipt, ValidatedAllocationLegality,
    ValidatedFixedPrecoloredIntervals, ValidatedLiveRanges, VirtualFixedConstraintSite,
    fixed_precolored_interval_plan_identity,
};

pub fn validate_fixed_precolored_intervals(
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    candidate: FixedPrecoloredIntervalPlan,
) -> Result<ValidatedFixedPrecoloredIntervals, FixedPrecoloredIntervalError> {
    if candidate.ranges != ranges.receipt().identity()
        || candidate.legality != legality.receipt().identity()
        || candidate.register_environment != legality.receipt().register_environment()
        || candidate.allocator_availability != legality.receipt().allocator_availability()
        || candidate.optimization_unit != ranges.receipt().optimization_unit()
        || candidate.fuel_schedule != ranges.receipt().fuel_schedule()
        || legality.receipt().ranges() != ranges.receipt().identity()
    {
        return Err(FixedPrecoloredIntervalError::RootMismatch);
    }
    let expected = super::replay::replay(ranges, legality, candidate.policy, candidate.budget)?;
    if candidate.usage != expected.usage {
        return Err(FixedPrecoloredIntervalError::UsageMismatch);
    }
    if candidate.functions != expected.functions
        || candidate.structural_unit_functions != expected.structural_unit_functions
    {
        return Err(FixedPrecoloredIntervalError::NonCanonicalFunctions);
    }
    let all = candidate
        .functions
        .iter()
        .chain(&candidate.structural_unit_functions)
        .flat_map(|function| &function.intervals)
        .collect::<Vec<_>>();
    let inspected_register_count = ranges
        .plan()
        .functions
        .iter()
        .chain(&ranges.plan().structural_unit_functions)
        .map(|function| function.virtual_registers.len())
        .sum();
    let entry_interval_count = all
        .iter()
        .filter(|row| matches!(row.site, VirtualFixedConstraintSite::Entry))
        .count();
    let receipt = FixedPrecoloredIntervalValidationReceipt {
        identity: fixed_precolored_interval_plan_identity(&candidate),
        ranges: candidate.ranges,
        legality: candidate.legality,
        register_environment: candidate.register_environment,
        allocator_availability: candidate.allocator_availability,
        optimization_unit: candidate.optimization_unit,
        fuel_schedule: candidate.fuel_schedule,
        policy: candidate.policy,
        usage: candidate.usage,
        function_count: candidate.functions.len(),
        structural_unit_function_count: candidate.structural_unit_functions.len(),
        inspected_register_count,
        interval_count: all.len(),
        entry_interval_count,
        operand_interval_count: all.len() - entry_interval_count,
    };
    Ok(ValidatedFixedPrecoloredIntervals {
        plan: candidate,
        receipt,
    })
}
