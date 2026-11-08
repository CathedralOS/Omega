/*
We join each target's physical register model, instruction constraint catalog,
and conservative reservation policy into a single validated allocator
environment (`ValidatedTargetRegisterEnvironment`). Rather than exposing a
separate decoded-environment replay path, we construct and validate the target
environment directly from ISA declarations (`baseline_target_register_environment`),
caching one validated instance per target process-wide while returning owned
copies to callers. We also derive the exact ABI preservation conventions and
preservation storage catalogs implied by the validated environment.
*/

/*
Register allocation and frame realization require an authoritative, closed join
between a target's physical register vocabulary, its instruction operand/clobber
constraints, its platform reservation overlays, and its selected instruction ABI
keys. By validating that join once at target setup and sealing the resulting
identity, we prevent cross-architecture mismatches, inapplicable platform
overlays (such as Darwin AArch64 reservations on ELF), or missing constraint
keys from reaching the allocator.
*/

/*
`catalog` selects the target's physical register model, constraint catalog,
baseline reservation profile, and ISA-owned selected constraint-key roster.
We validate the combined environment here alongside the ABI preservation
selection helpers (`selected_abi_preservation` and
`selected_preservation_storage_catalog`). These structures provide allocator and
frame-planning input without granting physical-home, machine-emission, or
publication authority.
*/

#![forbid(unsafe_code)]

mod catalog;
#[cfg(test)]
mod tests;

pub use register_model::FrameAbiPreservationConvention;

use catalog::{
    conservative_baseline_reservation_profile, selected_constraint_keys, target_constraint_catalog,
    target_physical_register_model,
};
use isa_aarch64::{
    Aarch64RegisterConstraintCatalogValidationError, aarch64_fixed_register_view,
    validate_aarch64_register_constraint_catalog,
};
use isa_x86_64::{
    X86_64RegisterConstraintCatalogValidationError, validate_x86_64_register_constraint_catalog,
    x86_64_fixed_register_view, x86_64_preservation_convention_for_target,
};
use register_model::{
    PreservationConvention, RegisterConstraintCatalog, RegisterConstraintKey,
    RegisterInstructionConstraint, RegisterModelValidationError, RegisterReservationProfile,
    RegisterReservationProfileValidationError, RegisterViewId,
    TargetRegisterEnvironmentConstraintKeys, TargetRegisterEnvironmentIdentity,
    ValidatedPhysicalRegisterModel, ValidatedPreservationStorageCatalog,
    ValidatedRegisterConstraintCatalog, ValidatedRegisterReservationProfile,
    target_register_environment_identity, validate_physical_register_model,
    validate_register_reservation_profile,
};
use selected_instructions::SelectedConstraintKeys;
use target::{Architecture, NativeTarget, ObjectFormat};
use target_operations::MachineRegister;

pub fn baseline_target_register_environment(
    target: NativeTarget,
) -> Result<ValidatedTargetRegisterEnvironment, TargetRegisterEnvironmentValidationError> {
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
    validate_target_register_environment_join(target, validated, constraints, reservations)
}

pub(crate) fn validate_target_register_environment_join(
    target: NativeTarget,
    physical: ValidatedPhysicalRegisterModel,
    constraints: RegisterConstraintCatalog,
    reservations: RegisterReservationProfile,
) -> Result<ValidatedTargetRegisterEnvironment, TargetRegisterEnvironmentValidationError> {
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
        &selected_keys.rows,
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
        self.constraints.constraint(key)
    }

    #[cfg(test)]
    pub(crate) fn scalar_call_constraint(&self) -> Option<&RegisterInstructionConstraint> {
        self.constraint(self::catalog::scalar_call_constraint_key(self.target)?)
    }

    pub fn selected_keys(&self) -> SelectedConstraintKeys {
        self.selected_keys.clone()
    }

    pub fn allocation_constraint_keys(&self) -> TargetRegisterEnvironmentConstraintKeys {
        self.selected_keys.rows.clone()
    }

    pub fn fixed_register_view(&self, register: MachineRegister) -> Option<RegisterViewId> {
        fixed_register_view(&self.physical, register)
    }
}

pub fn fixed_register_view(
    physical: &ValidatedPhysicalRegisterModel,
    register: MachineRegister,
) -> Option<RegisterViewId> {
    match physical.model().architecture {
        Architecture::X86_64 => x86_64_fixed_register_view(physical, register),
        Architecture::Aarch64 => aarch64_fixed_register_view(physical, register),
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SelectedAbiPreservation<'model> {
    pub kind: FrameAbiPreservationConvention,
    pub convention: &'model PreservationConvention,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbiPreservationSelectionError {
    UnsupportedTargetConvention,
}

pub fn selected_abi_preservation(
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<SelectedAbiPreservation<'_>, AbiPreservationSelectionError> {
    let target = environment.target();
    let (kind, convention) = match (target.architecture, target.object_format) {
        (Architecture::X86_64, ObjectFormat::Elf | ObjectFormat::MachO) => (
            FrameAbiPreservationConvention::SystemVAMD64,
            x86_64_preservation_convention_for_target(environment.physical(), target),
        ),
        (Architecture::X86_64, ObjectFormat::Coff) => (
            FrameAbiPreservationConvention::MicrosoftX64,
            x86_64_preservation_convention_for_target(environment.physical(), target),
        ),
        (Architecture::Aarch64, ObjectFormat::Elf) => (
            FrameAbiPreservationConvention::Aapcs64,
            isa_aarch64::aarch64_preservation_convention_for_target(environment.physical(), target),
        ),
        (Architecture::Aarch64, ObjectFormat::MachO) => (
            FrameAbiPreservationConvention::DarwinAapcs64,
            isa_aarch64::aarch64_preservation_convention_for_target(environment.physical(), target),
        ),
        _ => {
            return Err(AbiPreservationSelectionError::UnsupportedTargetConvention);
        }
    };
    convention
        .map(|convention| SelectedAbiPreservation { kind, convention })
        .ok_or(AbiPreservationSelectionError::UnsupportedTargetConvention)
}

pub fn selected_preservation_storage_catalog(
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<ValidatedPreservationStorageCatalog, AbiPreservationSelectionError> {
    match environment.target().architecture {
        Architecture::X86_64 => isa_x86_64::x86_64_preservation_storage_catalog(
            environment.physical(),
            environment.target(),
        )
        .map_err(|_| AbiPreservationSelectionError::UnsupportedTargetConvention),
        Architecture::Aarch64 => isa_aarch64::aarch64_preservation_storage_catalog(
            environment.physical(),
            environment.target(),
        )
        .map_err(|_| AbiPreservationSelectionError::UnsupportedTargetConvention),
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

#[cfg(test)]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;
