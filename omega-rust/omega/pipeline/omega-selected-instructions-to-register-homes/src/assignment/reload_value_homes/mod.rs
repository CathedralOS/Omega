//! Optimizer module role: executable entrance. Logical reload-value reanalysis and home assignment.
//!
//! This boundary proves one bounded block-local reload can occupy a physical
//! view after abstract spill scheduling. It creates no instruction, memory
//! effect, frame address, trap claim, encoding, or publication authority.

pub(in crate::assignment) mod compute;
mod identity;
mod model;
pub(in crate::assignment) mod replay;
mod validate;

pub use identity::reload_value_home_identity;
pub use model::*;
pub use validate::validate_reload_value_homes;

use crate::{
    ValidatedAbstractSpillInsertion, ValidatedAllocationLegality, ValidatedLiveRanges,
    ValidatedLogicalSpillOperations,
};

#[allow(clippy::too_many_arguments)]
pub fn assign_reload_value_homes(
    insertion: &ValidatedAbstractSpillInsertion,
    logical: &ValidatedLogicalSpillOperations,
    legality: &ValidatedAllocationLegality,
    ranges: &ValidatedLiveRanges,
    physical: &omega_register_model::ValidatedPhysicalRegisterModel,
    constraints: &omega_register_model::ValidatedRegisterConstraintCatalog,
    reservations: &omega_register_model::ValidatedRegisterReservationProfile,
    selected_keys: omega_register_model::TargetRegisterEnvironmentConstraintKeys,
    policy: ReloadValueHomePolicy,
    budget: omega_optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedReloadValueHomes, ReloadValueHomeError> {
    let plan = compute::compute(
        insertion,
        logical,
        legality,
        ranges,
        physical,
        constraints,
        reservations,
        selected_keys,
        policy,
        budget,
    )?;
    validate_reload_value_homes(
        insertion,
        logical,
        legality,
        ranges,
        physical,
        constraints,
        reservations,
        selected_keys,
        plan,
    )
}
