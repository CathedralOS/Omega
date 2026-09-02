//! Optimizer module role: stage group. Exact shared ABI preservation selection.
//!
//! This module selects immutable target convention data. It makes no frame,
//! save/restore, instruction, unwind, or publication decision.

mod model;

pub use model::FrameAbiPreservationConvention;
pub(crate) use model::{AbiPreservationSelectionError, SelectedAbiPreservation};

use crate::ValidatedTargetRegisterEnvironment;

pub(crate) fn selected_abi_preservation(
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<SelectedAbiPreservation<'_>, AbiPreservationSelectionError> {
    let target = environment.target();
    let (kind, convention) = match (target.architecture, target.object_format) {
        (omega_target::Architecture::X86_64, omega_target::ObjectFormat::Elf) => (
            FrameAbiPreservationConvention::SystemVAMD64,
            omega_isa_x86_64::x86_64_preservation_convention_for_target(
                environment.physical(),
                target,
            ),
        ),
        (omega_target::Architecture::X86_64, omega_target::ObjectFormat::Coff) => (
            FrameAbiPreservationConvention::MicrosoftX64,
            omega_isa_x86_64::x86_64_preservation_convention_for_target(
                environment.physical(),
                target,
            ),
        ),
        (omega_target::Architecture::Aarch64, omega_target::ObjectFormat::Elf) => (
            FrameAbiPreservationConvention::Aapcs64,
            omega_isa_aarch64::aarch64_preservation_convention_for_target(
                environment.physical(),
                target,
            ),
        ),
        (omega_target::Architecture::Aarch64, omega_target::ObjectFormat::MachO) => (
            FrameAbiPreservationConvention::DarwinAapcs64,
            omega_isa_aarch64::aarch64_preservation_convention_for_target(
                environment.physical(),
                target,
            ),
        ),
        _ => return Err(AbiPreservationSelectionError::UnsupportedTargetConvention),
    };
    convention
        .map(|convention| SelectedAbiPreservation { kind, convention })
        .ok_or(AbiPreservationSelectionError::UnsupportedTargetConvention)
}
