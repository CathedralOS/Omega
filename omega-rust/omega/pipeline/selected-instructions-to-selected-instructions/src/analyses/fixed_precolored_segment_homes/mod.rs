//! Optimizer module role: executable entrance. Fixed/precolored segment-home assignment.
//!
//! This boundary assigns one deterministic physical view to each authenticated
//! source-segment domain. It creates no copy, VReg, instruction, spill, or
//! transformed liveness, and distinct assigned views do not imply movement.

mod compute;
mod error;
mod identity;
mod model;
mod replay;
mod validation;

pub use error::FixedPrecoloredSegmentHomeError;
pub use identity::fixed_precolored_segment_home_plan_identity;
pub use model::*;
pub use validation::validate_fixed_precolored_segment_homes;

use register_model::{
    TargetRegisterEnvironmentConstraintKeys, TargetRegisterEnvironmentIdentity,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    ValidatedRegisterReservationProfile,
};

#[allow(clippy::too_many_arguments)]
pub fn assign_fixed_precolored_segment_homes(
    ranges: &crate::ValidatedLiveRanges,
    legality: &crate::ValidatedAllocationLegality,
    fixed: &crate::ValidatedFixedPrecoloredIntervals,
    requirements: &crate::ValidatedFixedPrecoloredSplitRequirements,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: &TargetRegisterEnvironmentConstraintKeys,
    policy: FixedPrecoloredSegmentHomePolicy,
    budget: optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedFixedPrecoloredSegmentHomes, FixedPrecoloredSegmentHomeError> {
    let plan = compute::compute(
        ranges,
        legality,
        fixed,
        requirements,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
        policy,
        budget,
    )?;
    validate_fixed_precolored_segment_homes(
        ranges,
        legality,
        fixed,
        requirements,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
        plan,
    )
}
