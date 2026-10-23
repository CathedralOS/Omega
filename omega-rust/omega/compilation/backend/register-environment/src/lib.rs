#![forbid(unsafe_code)]

//! Optimizer module role: executable entrance. Backend-owned register environment setup.
//!
//! Joins exact ISA/ABI declarations and reservation policy into an independently
//! validated allocator environment. This is shared target setup, not a program
//! transformation or a successor in the representation pipeline.
//!
//! `baseline_target_register_environment` is the setup entry. There is no
//! decoded-environment replay API: allocation consumes the constructed model.
//! `catalog` selects the target's physical register model, constraint catalog
//! and baseline reservation profile and resolves the target's selected
//! constraint-key roster from its ISA owner; `validation` joins them into the
//! `ValidatedTargetRegisterEnvironment` declared here; `abi_preservation`
//! selects the ABI preservation facts the validated environment implies.

mod abi_preservation;
mod catalog;
mod validation;

pub use abi_preservation::{
    FrameAbiPreservationConvention, selected_abi_preservation,
    selected_preservation_storage_catalog,
};
#[cfg(test)]
mod tests;

use register_model::validate_physical_register_model;
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

/// Construct the target-owned allocator environment from its ISA declarations.
/// Decoded or cached register models have no separate replay entrance here.
pub fn baseline_target_register_environment(
    target: NativeTarget,
) -> Result<ValidatedTargetRegisterEnvironment, TargetRegisterEnvironmentValidationError> {
    // The baseline is built from the target's constant declarations alone, so
    // one build and validation per target serves every later request in the
    // process; each caller still receives its own copy.
    static BUILT: std::sync::Mutex<
        Vec<(
            NativeTarget,
            Result<ValidatedTargetRegisterEnvironment, TargetRegisterEnvironmentValidationError>,
        )>,
    > = std::sync::Mutex::new(Vec::new());
    if let Ok(built) = BUILT.lock()
        && let Some((_, environment)) = built.iter().find(|(candidate, _)| *candidate == target)
    {
        return environment.clone();
    }
    let environment = build_baseline_target_register_environment(target);
    if let Ok(mut built) = BUILT.lock()
        && !built.iter().any(|(candidate, _)| *candidate == target)
    {
        built.push((target, environment.clone()));
    }
    environment
}

fn build_baseline_target_register_environment(
    target: NativeTarget,
) -> Result<ValidatedTargetRegisterEnvironment, TargetRegisterEnvironmentValidationError> {
    let physical = target_physical_register_model(target);
    let validated = validate_physical_register_model(physical.clone())
        .map_err(TargetRegisterEnvironmentValidationError::Physical)?;
    let constraints = target_constraint_catalog(target, &validated);
    let reservations = conservative_baseline_reservation_profile(target, &physical);
    validation::validate_target_register_environment_join(
        target,
        validated,
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

    /// Whether an exact scalar pointer access can retain its displacement in
    /// the native load/store. Otherwise selection must form an address first;
    /// this does not change the access's semantic place or byte extent.
    pub const fn admits_pointer_displacement(&self, byte_offset: u32, byte_size: u32) -> bool {
        if !matches!(byte_size, 1 | 2 | 4 | 8) {
            return false;
        }
        match self.target.architecture {
            Architecture::X86_64 => byte_offset <= i32::MAX as u32,
            Architecture::Aarch64 => {
                byte_offset.is_multiple_of(byte_size) && byte_offset / byte_size <= 4095
            }
        }
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
    pub(crate) fn scalar_call_constraint(&self) -> Option<&RegisterInstructionConstraint> {
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
        fixed_register_view(&self.physical, register)
    }
}

/// Resolve an ABI register through its ISA-owned physical vocabulary.
pub fn fixed_register_view(
    physical: &ValidatedPhysicalRegisterModel,
    register: target_operations::MachineRegister,
) -> Option<register_model::RegisterViewId> {
    match physical.model().architecture {
        Architecture::X86_64 => x86_64_fixed_register_view(physical, register),
        Architecture::Aarch64 => aarch64_fixed_register_view(physical, register),
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

// The unit tests allocate heavily across nextest's parallel processes;
// mimalloc avoids the system allocator's zone locks and free lists.
// Test-only: the product keeps the system allocator.
#[cfg(test)]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;
