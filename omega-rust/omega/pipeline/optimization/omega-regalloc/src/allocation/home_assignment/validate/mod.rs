//! Optimizer module role: executable entrance. Independent register-home validation entrance.
//!
//! Root custody, tied-domain reconstruction, conflict replay, and receipt
//! construction remain separate from the producer's assignment mechanics.

mod conflicts;
mod domain;
mod receipt;
mod replay;
mod root;

use omega_register_model::{
    TargetRegisterEnvironmentConstraintKeys, TargetRegisterEnvironmentIdentity,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    ValidatedRegisterReservationProfile,
};

use crate::{
    RegisterHomeError, RegisterHomePlan, ValidatedAllocationLegality, ValidatedLiveRanges,
    ValidatedRegisterHomes,
};

#[allow(unused_imports)]
pub(crate) use replay::replay_function;
#[cfg(test)]
pub(in crate::allocation::home_assignment) use replay::validate_function;

#[allow(clippy::too_many_arguments)]
pub fn validate_register_homes(
    legality: &ValidatedAllocationLegality,
    ranges: &ValidatedLiveRanges,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
    plan: RegisterHomePlan,
) -> Result<ValidatedRegisterHomes, RegisterHomeError> {
    root::validate(
        legality,
        ranges,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
        &plan,
    )?;
    for (function_index, ((actual, legality), ranges)) in plan
        .functions
        .iter()
        .zip(&legality.plan().functions)
        .zip(&ranges.plan().functions)
        .enumerate()
    {
        replay::validate_function(function_index, actual, legality, ranges, physical)?;
    }
    let validation_receipt = receipt::build(&plan, ranges);
    Ok(ValidatedRegisterHomes {
        plan,
        receipt: validation_receipt,
    })
}
