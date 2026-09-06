//! Optimizer module role: executable entrance. Exact fixed-view copy insertion and independent CFG replay entrance.

use target_operations_to_selected_instructions::ValidatedSelectedInstructions;

use crate::*;

pub(crate) mod codec;
pub(crate) mod compute;
mod evidence;
pub(crate) mod identity;
pub(crate) mod model;
pub(crate) mod validate;
mod work;

pub use identity::fixed_view_copy_identity;
pub use model::*;
pub use validate::validate_fixed_view_copies;

/// Apply one explicitly selected fixed-view copy policy and independently
/// reconstruct its complete selected CFG.
pub fn materialize_fixed_view_copies(
    selected: &ValidatedSelectedInstructions,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    fixed: &ValidatedFixedPrecoloredIntervals,
    requirements: &ValidatedFixedPrecoloredSplitRequirements,
    homes: &ValidatedFixedPrecoloredSegmentHomes,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: &TargetRegisterEnvironmentConstraintKeys,
    policy: FixedViewCopyPolicy,
    budget: optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedFixedViewCopies, FixedViewCopyError> {
    let plan = compute::compute_terminal_fixed_view_copies(
        selected,
        ranges,
        legality,
        fixed,
        requirements,
        homes,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
        policy,
        budget,
    )?;
    validate_fixed_view_copies(
        selected,
        ranges,
        legality,
        fixed,
        requirements,
        homes,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
        plan,
    )
}
