//! Exact ABI preservation facts selected by a validated target environment.
//!
//! These are immutable target facts. They grant no allocation, frame,
//! save/restore instruction, unwind, or publication authority.

use register_model::{PreservationConvention, ValidatedPreservationStorageCatalog};

use crate::ValidatedTargetRegisterEnvironment;

pub use register_model::FrameAbiPreservationConvention;

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
        (target::Architecture::X86_64, target::ObjectFormat::Elf) => (
            FrameAbiPreservationConvention::SystemVAMD64,
            isa_x86_64::x86_64_preservation_convention_for_target(environment.physical(), target),
        ),
        (target::Architecture::X86_64, target::ObjectFormat::Coff) => (
            FrameAbiPreservationConvention::MicrosoftX64,
            isa_x86_64::x86_64_preservation_convention_for_target(environment.physical(), target),
        ),
        (target::Architecture::Aarch64, target::ObjectFormat::Elf) => (
            FrameAbiPreservationConvention::Aapcs64,
            isa_aarch64::aarch64_preservation_convention_for_target(environment.physical(), target),
        ),
        (target::Architecture::Aarch64, target::ObjectFormat::MachO) => (
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

/// Select the independently validated target-owned storage grouping paired
/// with the target's ABI preservation convention.
pub fn selected_preservation_storage_catalog(
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<ValidatedPreservationStorageCatalog, AbiPreservationSelectionError> {
    match environment.target().architecture {
        target::Architecture::X86_64 => isa_x86_64::x86_64_preservation_storage_catalog(
            environment.physical(),
            environment.target(),
        )
        .map_err(|_| AbiPreservationSelectionError::UnsupportedTargetConvention),
        target::Architecture::Aarch64 => isa_aarch64::aarch64_preservation_storage_catalog(
            environment.physical(),
            environment.target(),
        )
        .map_err(|_| AbiPreservationSelectionError::UnsupportedTargetConvention),
    }
}
