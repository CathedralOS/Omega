use isa_aarch64::{Aarch64RegisterConstraintCatalogValidationError, aarch64_fixed_register_view};
use isa_x86_64::{X86_64RegisterConstraintCatalogValidationError, x86_64_fixed_register_view};
use register_model::{
    RegisterConstraintKey, RegisterInstructionConstraint, RegisterModelValidationError,
    RegisterReservationProfileValidationError, TargetRegisterEnvironmentConstraintKeys,
    TargetRegisterEnvironmentIdentity, ValidatedPhysicalRegisterModel,
    ValidatedRegisterConstraintCatalog, ValidatedRegisterReservationProfile,
};
use selected_instructions::SelectedConstraintKeys;
use target::{Architecture, NativeTarget};

use super::catalog::selected_environment_keys;

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
    pub(super) const fn new(
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
        self.constraint(super::catalog::scalar_call_constraint_key(self.target)?)
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
