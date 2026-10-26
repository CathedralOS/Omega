use crate::{
    FixedPrecoloredSegmentHomeError, FixedPrecoloredSegmentHomeValidationReceipt,
    ValidatedFixedPrecoloredSegmentHomes,
};
use register_homes::{FixedPrecoloredSegmentHomePlan, fixed_precolored_segment_home_plan_identity};
/// Admit a freshly produced plan with its receipt; the producer calls this
/// instead of replaying its own output.
pub(crate) fn seal_fixed_precolored_segment_homes(
    candidate: FixedPrecoloredSegmentHomePlan,
) -> Result<ValidatedFixedPrecoloredSegmentHomes, FixedPrecoloredSegmentHomeError> {
    let receipt = FixedPrecoloredSegmentHomeValidationReceipt {
        identity: fixed_precolored_segment_home_plan_identity(&candidate),
        split_requirements: candidate.split_requirements,
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
    Ok(ValidatedFixedPrecoloredSegmentHomes {
        plan: candidate,
        receipt,
    })
}
