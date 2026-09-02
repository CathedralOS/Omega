use omega_register_model::{
    TargetRegisterEnvironmentConstraintKeys, TargetRegisterEnvironmentIdentity,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    ValidatedRegisterReservationProfile, target_register_environment_identity,
};

use crate::{
    FixedPrecoloredIntervalPolicy, FixedPrecoloredSegmentHomeError,
    FixedPrecoloredSplitRequirementPolicy, ValidatedAllocationLegality,
    ValidatedFixedPrecoloredIntervals, ValidatedFixedPrecoloredSplitRequirements,
    ValidatedLiveRanges,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn validate(
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    fixed: &ValidatedFixedPrecoloredIntervals,
    requirements: &ValidatedFixedPrecoloredSplitRequirements,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
) -> Result<(), FixedPrecoloredSegmentHomeError> {
    if requirements.receipt().fixed_intervals() != fixed.receipt().identity()
        || requirements.receipt().ranges() != ranges.receipt().identity()
        || requirements.receipt().legality() != legality.receipt().identity()
        || requirements.receipt().register_environment() != register_environment
        || requirements.receipt().allocator_availability()
            != legality.receipt().allocator_availability()
        || requirements.receipt().optimization_unit() != ranges.receipt().optimization_unit()
        || requirements.receipt().fuel_schedule() != ranges.receipt().fuel_schedule()
        || requirements.receipt().target() != ranges.plan().target
        || requirements.receipt().policy()
            != FixedPrecoloredSplitRequirementPolicy::FixedUseBoundaryRequirementsV1
        || fixed.receipt().policy()
            != FixedPrecoloredIntervalPolicy::FixedConstraintPointIntervalsV1
        || legality.receipt().ranges() != ranges.receipt().identity()
        || constraints.physical_identity() != physical.identity()
        || reservations.physical_identity() != physical.identity()
        || reservations.target() != ranges.plan().target
        || target_register_environment_identity(
            ranges.plan().target,
            physical,
            constraints,
            reservations,
            selected_keys,
        ) != register_environment
    {
        return Err(FixedPrecoloredSegmentHomeError::RootMismatch);
    }
    Ok(())
}
