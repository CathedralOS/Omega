#![forbid(unsafe_code)]

//! Optimizer module role: executable entrance. Backend-owned register environment setup.
//!
//! Joins exact ISA/ABI declarations and reservation policy into an independently
//! validated allocator environment. This is shared target setup, not a program
//! transformation or a successor in the representation pipeline.
//!
//! `baseline_target_register_environment` and
//! `validate_target_register_environment` in this file are the entries.
//! `catalog` selects the target's physical register model, constraint catalog
//! and baseline reservation profile; `validation` joins them into the
//! `ValidatedTargetRegisterEnvironment` declared here; `abi_preservation`
//! selects the ABI preservation facts the validated environment implies.

mod abi_preservation;
mod catalog;
mod validation;

pub use abi_preservation::{
    AbiPreservationSelectionError, FrameAbiPreservationConvention, SelectedAbiPreservation,
    selected_abi_preservation, selected_preservation_storage_catalog,
};

#[cfg(test)]
mod tests;

use register_model::{
    PhysicalRegisterModel, RegisterConstraintCatalog, RegisterReservationProfile,
    validate_physical_register_model,
};
use target::NativeTarget;

use catalog::selected_environment_keys;
use catalog::{
    conservative_baseline_reservation_profile, target_constraint_catalog,
    target_physical_register_model,
};
use isa_aarch64::{Aarch64RegisterConstraintCatalogValidationError, aarch64_fixed_register_view};
use isa_x86_64::{X86_64RegisterConstraintCatalogValidationError, x86_64_fixed_register_view};
use register_model::{
    RegisterConstraintKey, RegisterInstructionConstraint, RegisterModelValidationError,
    RegisterReservationProfileValidationError, TargetRegisterEnvironmentConstraintKeys,
    TargetRegisterEnvironmentIdentity, ValidatedPhysicalRegisterModel,
    ValidatedRegisterConstraintCatalog, ValidatedRegisterReservationProfile,
};
use selected_instructions::SelectedConstraintKeys;
use target::Architecture;

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

/// Clean-lane custody of the exact target, independently validated physical
/// register model, and target-semantic instruction constraint catalog.
///
/// This is allocator input, not allocator output. It grants no physical-home,
/// machine-emission, or publication authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedTargetRegisterEnvironment {
    target: NativeTarget,
    physical: ValidatedPhysicalRegisterModel,
    constraints: ValidatedRegisterConstraintCatalog,
    reservations: ValidatedRegisterReservationProfile,
    selected_keys: SelectedConstraintKeys,
    identity: TargetRegisterEnvironmentIdentity,
}

impl ValidatedTargetRegisterEnvironment {
    const fn new(
        target: NativeTarget,
        physical: ValidatedPhysicalRegisterModel,
        constraints: ValidatedRegisterConstraintCatalog,
        reservations: ValidatedRegisterReservationProfile,
        selected_keys: SelectedConstraintKeys,
        identity: TargetRegisterEnvironmentIdentity,
    ) -> Self {
        Self {
            target,
            physical,
            constraints,
            reservations,
            selected_keys,
            identity,
        }
    }

    pub const fn target(&self) -> NativeTarget {
        self.target
    }

    pub const fn physical(&self) -> &ValidatedPhysicalRegisterModel {
        &self.physical
    }

    pub const fn constraints(&self) -> &ValidatedRegisterConstraintCatalog {
        &self.constraints
    }

    pub const fn reservations(&self) -> &ValidatedRegisterReservationProfile {
        &self.reservations
    }

    pub const fn identity(&self) -> TargetRegisterEnvironmentIdentity {
        self.identity
    }

    pub fn constraint(&self, key: RegisterConstraintKey) -> Option<&RegisterInstructionConstraint> {
        self.constraints
            .catalog()
            .constraints
            .iter()
            .find(|constraint| constraint.key == key)
    }

    /// Target-selected scalar-call constraint. This is validated environment
    /// data only; it does not claim that the selected CFG can lower a general
    /// scalar call yet.
    pub fn scalar_call_constraint(&self) -> Option<&RegisterInstructionConstraint> {
        self.constraint(self::catalog::scalar_call_constraint_key(self.target)?)
    }

    pub fn selected_keys(&self) -> SelectedConstraintKeys {
        self.selected_keys.clone()
    }

    pub fn allocation_constraint_keys(&self) -> TargetRegisterEnvironmentConstraintKeys {
        selected_environment_keys(self.selected_keys.clone())
    }

    pub fn fixed_register_view(
        &self,
        register: target_operations::MachineRegister,
    ) -> Option<register_model::RegisterViewId> {
        match self.target.architecture {
            Architecture::X86_64 => x86_64_fixed_register_view(&self.physical, register),
            Architecture::Aarch64 => aarch64_fixed_register_view(&self.physical, register),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetRegisterEnvironmentValidationError {
    Physical(RegisterModelValidationError),
    TargetArchitectureMismatch {
        target: Architecture,
        model: Architecture,
    },
    X86_64(X86_64RegisterConstraintCatalogValidationError),
    Aarch64(Aarch64RegisterConstraintCatalogValidationError),
    Reservations(RegisterReservationProfileValidationError),
    InapplicableReservationOverlay,
    UnsupportedSelectedInstructionAbi,
}

impl std::fmt::Display for TargetRegisterEnvironmentValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid target register environment: {self:?}")
    }
}

impl std::error::Error for TargetRegisterEnvironmentValidationError {}
