use register_model::{
    TargetRegisterEnvironmentConstraintKeys, TargetRegisterEnvironmentIdentity,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    ValidatedRegisterReservationProfile,
};
use target_operations_to_selected_instructions::ValidatedSelectedInstructions;

use crate::{
    FixedViewCopyError, FixedViewCopyPlan, ValidatedAllocationLegality, ValidatedLiveRanges,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_roots(
    selected: &ValidatedSelectedInstructions,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: &TargetRegisterEnvironmentConstraintKeys,
    plan: &FixedViewCopyPlan,
) -> Result<(), FixedViewCopyError> {
    if plan.source_selected != selected.receipt().identity()
        || plan.source_ranges != ranges.receipt().identity()
        || plan.source_legality != legality.receipt().identity()
        || plan.register_environment != register_environment
        || plan.allocator_availability != legality.receipt().allocator_availability()
    {
        return Err(FixedViewCopyError::RootMismatch);
    }
    super::super::work::roots_align(
        selected,
        ranges,
        legality,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
    )
}
