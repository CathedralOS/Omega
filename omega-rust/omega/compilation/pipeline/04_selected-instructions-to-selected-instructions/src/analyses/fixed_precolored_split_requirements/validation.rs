use crate::{
    FixedPrecoloredSplitRequirementError, FixedPrecoloredSplitRequirementValidationReceipt,
    ValidatedAllocationLegality, ValidatedFixedPrecoloredSplitRequirements,
};
use register_homes::{
    FixedPrecoloredSourceSegmentOpening, FixedPrecoloredSplitRequirementPlan,
    fixed_precolored_split_requirement_plan_identity,
};
/// Admit a freshly produced plan with its receipt; the producer calls this
/// instead of replaying its own output.
pub(crate) fn seal_fixed_precolored_split_requirements(
    legality: &ValidatedAllocationLegality,
    candidate: FixedPrecoloredSplitRequirementPlan,
) -> Result<ValidatedFixedPrecoloredSplitRequirements, FixedPrecoloredSplitRequirementError> {
    let registers = candidate
        .functions
        .iter()
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
