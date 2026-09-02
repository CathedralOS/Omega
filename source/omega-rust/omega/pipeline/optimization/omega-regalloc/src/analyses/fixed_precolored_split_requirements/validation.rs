use crate::{
    FixedPrecoloredIntervalPolicy, FixedPrecoloredSourceSegmentOpening,
    FixedPrecoloredSplitRequirementError, FixedPrecoloredSplitRequirementPlan,
    FixedPrecoloredSplitRequirementValidationReceipt, ValidatedAllocationLegality,
    ValidatedFixedPrecoloredIntervals, ValidatedFixedPrecoloredSplitRequirements,
    ValidatedLiveRanges, fixed_precolored_split_requirement_plan_identity,
};

pub fn validate_fixed_precolored_split_requirements(
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    fixed: &ValidatedFixedPrecoloredIntervals,
    candidate: FixedPrecoloredSplitRequirementPlan,
) -> Result<ValidatedFixedPrecoloredSplitRequirements, FixedPrecoloredSplitRequirementError> {
    if candidate.fixed_intervals != fixed.receipt().identity()
        || candidate.ranges != ranges.receipt().identity()
        || candidate.legality != legality.receipt().identity()
        || candidate.register_environment != legality.receipt().register_environment()
        || candidate.allocator_availability != legality.receipt().allocator_availability()
        || candidate.optimization_unit != ranges.receipt().optimization_unit()
        || candidate.fuel_schedule != ranges.receipt().fuel_schedule()
        || candidate.target != ranges.plan().target
        || legality.receipt().ranges() != ranges.receipt().identity()
        || fixed.receipt().ranges() != ranges.receipt().identity()
        || fixed.receipt().legality() != legality.receipt().identity()
        || fixed.receipt().policy()
            != FixedPrecoloredIntervalPolicy::FixedConstraintPointIntervalsV1
    {
        return Err(FixedPrecoloredSplitRequirementError::RootMismatch);
    }
    let expected =
        super::replay::replay(ranges, legality, fixed, candidate.policy, candidate.budget)?;
    if candidate.usage != expected.usage {
        return Err(FixedPrecoloredSplitRequirementError::UsageMismatch);
    }
    if candidate.functions != expected.functions
        || candidate.structural_unit_functions != expected.structural_unit_functions
    {
        return Err(FixedPrecoloredSplitRequirementError::NonCanonicalFunctions);
    }
    let registers = candidate
        .functions
        .iter()
        .chain(&candidate.structural_unit_functions)
        .flat_map(|function| &function.registers)
        .collect::<Vec<_>>();
    let fragments = registers
        .iter()
        .flat_map(|register| &register.fragments)
        .collect::<Vec<_>>();
    let segment_count = fragments
        .iter()
        .map(|fragment| fragment.segments.len())
        .sum();
    let incompatible_fixed_use_boundary_count = fragments
        .iter()
        .flat_map(|fragment| &fragment.segments)
        .filter(|segment| {
            matches!(
                segment.opening,
                FixedPrecoloredSourceSegmentOpening::IncompatibleFixedUseDomainBoundaryV1 { .. }
            )
        })
        .count();
    let receipt = FixedPrecoloredSplitRequirementValidationReceipt {
        identity: fixed_precolored_split_requirement_plan_identity(&candidate),
        fixed_intervals: candidate.fixed_intervals,
        ranges: candidate.ranges,
        legality: candidate.legality,
        register_environment: candidate.register_environment,
        allocator_availability: candidate.allocator_availability,
        optimization_unit: candidate.optimization_unit,
        fuel_schedule: candidate.fuel_schedule,
        target: candidate.target,
        policy: candidate.policy,
        usage: candidate.usage,
        function_count: candidate.functions.len(),
        structural_unit_function_count: candidate.structural_unit_functions.len(),
        register_count: registers.len(),
        fragment_count: fragments.len(),
        source_point_count: legality.receipt().point_count(),
        segment_count,
        incompatible_fixed_use_boundary_count,
    };
    Ok(ValidatedFixedPrecoloredSplitRequirements {
        plan: candidate,
        receipt,
    })
}
