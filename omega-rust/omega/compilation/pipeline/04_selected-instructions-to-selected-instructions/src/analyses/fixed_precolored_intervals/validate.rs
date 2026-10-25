//! Independent replay comparison and fixed/precolored receipt sealing.

use crate::{
    FixedPrecoloredIntervalError, FixedPrecoloredIntervalValidationReceipt,
    ValidatedFixedPrecoloredIntervals,
};
use register_homes::{FixedPrecoloredIntervalPlan, fixed_precolored_interval_plan_identity};
/// Admit a freshly produced plan with its receipt; the producer calls this
/// instead of replaying its own output.
pub(crate) fn seal_fixed_precolored_intervals(
    candidate: FixedPrecoloredIntervalPlan,
) -> Result<ValidatedFixedPrecoloredIntervals, FixedPrecoloredIntervalError> {
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
    };
    Ok(ValidatedFixedPrecoloredIntervals {
        plan: candidate,
        receipt,
    })
}
