use omega_isa_aarch64::validate_aarch64_register_constraint_catalog;
use omega_isa_x86_64::validate_x86_64_register_constraint_catalog;
use omega_register_model::{
    PhysicalRegisterModel, RegisterConstraintCatalog, RegisterReservationProfile,
    target_register_environment_identity, validate_physical_register_model,
    validate_register_reservation_profile,
};
use omega_target::{Architecture, NativeTarget, ObjectFormat};

use super::catalog::{selected_constraint_keys, selected_environment_keys};
use super::model::{TargetRegisterEnvironmentValidationError, ValidatedTargetRegisterEnvironment};

pub(super) fn validate_target_register_environment_join(
    target: NativeTarget,
    physical: PhysicalRegisterModel,
    constraints: RegisterConstraintCatalog,
    reservations: RegisterReservationProfile,
) -> Result<ValidatedTargetRegisterEnvironment, TargetRegisterEnvironmentValidationError> {
    let physical = validate_physical_register_model(physical)
        .map_err(TargetRegisterEnvironmentValidationError::Physical)?;
    if target.architecture != physical.model().architecture {
        return Err(
            TargetRegisterEnvironmentValidationError::TargetArchitectureMismatch {
                target: target.architecture,
                model: physical.model().architecture,
            },
        );
    }
    let constraints = match target.architecture {
        Architecture::X86_64 => validate_x86_64_register_constraint_catalog(constraints, &physical)
            .map_err(TargetRegisterEnvironmentValidationError::X86_64)?,
        Architecture::Aarch64 => {
            validate_aarch64_register_constraint_catalog(constraints, &physical)
                .map_err(TargetRegisterEnvironmentValidationError::Aarch64)?
        }
    };
    let selected_keys = selected_constraint_keys(target)
        .ok_or(TargetRegisterEnvironmentValidationError::UnsupportedSelectedInstructionAbi)?;
    if reservations
        .active_overlays
        .iter()
        .any(|name| name == "darwin.aarch64.platform")
        && target.object_format != ObjectFormat::MachO
    {
        return Err(TargetRegisterEnvironmentValidationError::InapplicableReservationOverlay);
    }
    let reservations = validate_register_reservation_profile(reservations, target, &physical)
        .map_err(TargetRegisterEnvironmentValidationError::Reservations)?;
    let identity = target_register_environment_identity(
        target,
        &physical,
        &constraints,
        &reservations,
        selected_environment_keys(selected_keys),
    );
    Ok(ValidatedTargetRegisterEnvironment::new(
        target,
        physical,
        constraints,
        reservations,
        selected_keys,
        identity,
    ))
}
