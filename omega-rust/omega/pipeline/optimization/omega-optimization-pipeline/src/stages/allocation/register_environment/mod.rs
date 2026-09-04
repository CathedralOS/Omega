//! Optimizer module role: executable entrance. Target register-environment construction and validation stage.
//!
//! This entrance is the visible join between exact target declarations,
//! reservation policy, and the independently validated allocator environment.

mod catalog;
mod model;
mod validation;

#[cfg(test)]
mod tests;

use omega_register_model::{
    PhysicalRegisterModel, RegisterConstraintCatalog, RegisterReservationProfile,
    validate_physical_register_model,
};
use omega_target::NativeTarget;

use catalog::{
    conservative_baseline_reservation_profile, target_constraint_catalog,
    target_physical_register_model,
};
pub use model::{TargetRegisterEnvironmentValidationError, ValidatedTargetRegisterEnvironment};

/// Build the current target-owned baseline environment and pass it through the
/// same independently callable join validator used for decoded/cached models.
pub fn baseline_target_register_environment(
    target: NativeTarget,
) -> Result<ValidatedTargetRegisterEnvironment, TargetRegisterEnvironmentValidationError> {
    let physical = target_physical_register_model(target);
    let validated = validate_physical_register_model(physical.clone())
        .map_err(TargetRegisterEnvironmentValidationError::Physical)?;
    let constraints = target_constraint_catalog(target, &validated);
    let reservations = conservative_baseline_reservation_profile(target, &physical);
    validation::validate_target_register_environment_join(
        target,
        physical,
        constraints,
        reservations,
    )
}

/// Independently join raw physical and constraint declarations for one exact
/// native target. The architecture-neutral structural validators run first;
/// the selected clean ISA owner then checks every target-semantic row.
pub fn validate_target_register_environment(
    target: NativeTarget,
    physical: PhysicalRegisterModel,
    constraints: RegisterConstraintCatalog,
) -> Result<ValidatedTargetRegisterEnvironment, TargetRegisterEnvironmentValidationError> {
    let reservations = conservative_baseline_reservation_profile(target, &physical);
    validation::validate_target_register_environment_join(
        target,
        physical,
        constraints,
        reservations,
    )
}

/// Join the exact raw artifacts and an explicit active reservation profile.
/// This is the cache/decode validation boundary; no reservation declaration
/// becomes active merely by appearing in the physical model.
pub fn validate_target_register_environment_with_reservations(
    target: NativeTarget,
    physical: PhysicalRegisterModel,
    constraints: RegisterConstraintCatalog,
    reservations: RegisterReservationProfile,
) -> Result<ValidatedTargetRegisterEnvironment, TargetRegisterEnvironmentValidationError> {
    validation::validate_target_register_environment_join(
        target,
        physical,
        constraints,
        reservations,
    )
}
