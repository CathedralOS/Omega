use crate::{
    FixedPrecoloredSplitRequirementError, FixedPrecoloredSplitRequirementValidationReceipt,
    ValidatedFixedPrecoloredSplitRequirements,
};
use register_homes::{
    FixedPrecoloredSplitRequirementPlan, fixed_precolored_split_requirement_plan_identity,
};
/// Admit a freshly produced plan with its receipt; the producer calls this
/// instead of replaying its own output.
pub(crate) fn seal_fixed_precolored_split_requirements(
    candidate: FixedPrecoloredSplitRequirementPlan,
) -> Result<ValidatedFixedPrecoloredSplitRequirements, FixedPrecoloredSplitRequirementError> {
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
    };
    Ok(ValidatedFixedPrecoloredSplitRequirements {
        plan: candidate,
        receipt,
    })
}
