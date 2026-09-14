//! Optimizer module role: executable entrance. Epoch-zero/one reload-home reanalysis.
//!
//! This join replays allocation after generalized abstract spill scheduling and
//! assigns physical views to the two logical reload actions. It creates no
//! virtual register, instruction, memory effect, frame, trap, or publication
//! authority.

mod compute;
mod identity;
mod model;
mod replay;
mod validate;

pub use identity::generalized_reload_value_home_identity;
pub use model::*;
pub use validate::validate_generalized_reload_value_homes;

use crate::{
    ValidatedAbstractSpillInsertion, ValidatedAllocationLegality,
    ValidatedGeneralizedSpillInsertion, ValidatedLiveRanges, ValidatedSpillRecoveryActions,
};

#[allow(clippy::too_many_arguments)]
pub fn assign_generalized_reload_value_homes(
    generalized: &ValidatedGeneralizedSpillInsertion,
    first: &ValidatedAbstractSpillInsertion,
    second: &ValidatedSpillRecoveryActions,
    selected: &target_operations_to_selected_instructions::ValidatedSelectedInstructions,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    physical: &register_model::ValidatedPhysicalRegisterModel,
    constraints: &register_model::ValidatedRegisterConstraintCatalog,
    reservations: &register_model::ValidatedRegisterReservationProfile,
    selected_keys: &register_model::TargetRegisterEnvironmentConstraintKeys,
    policy: GeneralizedReloadValueHomePolicy,
    budget: optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedGeneralizedReloadValueHomes, GeneralizedReloadValueHomeError> {
    let plan = compute::compute(
        generalized,
        first,
        second,
        selected,
        ranges,
        legality,
        physical,
        constraints,
        reservations,
        selected_keys,
        policy,
        budget,
    )?;
    validate_generalized_reload_value_homes(
        generalized,
        first,
        second,
        selected,
        ranges,
        legality,
        physical,
        constraints,
        reservations,
        selected_keys,
        plan,
    )
}
