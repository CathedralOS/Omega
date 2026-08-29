//! Ranked native-fuel replay coordination.
//!
//! Object admission reconstructs the equivalent branch rebasing. Publication
//! separately rejoins that custody to the exact final bytes.

mod coordinates;
mod object;
mod publication;

use omega_machine_code::{MachineCodeFunction, NativeFuelRankedU32CountdownRebaseRecord};
use omega_target::NativeTarget;
use psi_core::MachineId;

use crate::{NativeFuelValidationError, ObjectArtifact, ValidatedNativeFuelArtifact};

pub(super) fn classify(artifact: &ObjectArtifact) -> Option<MachineId> {
    artifact
        .functions()
        .iter()
        .find(|function| function.ranked_u32_countdown.is_some())
        .map(|function| function.machine)
}

pub(super) fn replay_rebased_branches(
    target: NativeTarget,
    source: &MachineCodeFunction,
    expected: &mut [u8],
    supplied: &[u8],
) -> Result<NativeFuelRankedU32CountdownRebaseRecord, NativeFuelValidationError> {
    let invalid = NativeFuelValidationError::InvalidRankedCountdownRebasing(source.machine);
    let record = coordinates::reconstruct(target, &source.bytes, &source.fuel_attribution)
        .ok_or_else(|| invalid.clone())?;
    object::admit_rebased_branches(target, source.machine, expected, supplied, record)?;
    Ok(record)
}

pub(super) fn replay_final_image(
    artifact: &ValidatedNativeFuelArtifact,
    final_text: &[u8],
) -> Result<(), psi_diagnostics::Diagnostic> {
    crate::ranked_u32_countdown::replay_ranked_u32_countdown_final_image(
        artifact.semantic_artifact(),
    )?;
    publication::replay_metered_final_image(artifact, final_text)
}
