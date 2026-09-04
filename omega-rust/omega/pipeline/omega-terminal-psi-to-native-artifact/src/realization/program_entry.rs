//! Checked ProgramEntry settlement before ordinary native realization.

use psi_checked_trees_to_terminal::ProducedProgramEntryTerminalArtifact;
use psi_diagnostics::Diagnostic;

use crate::entry_settlement::validate_native_program_entry_settlement;

use super::diagnostics::realization_error;
use super::{
    NativeRealizationRequest, SettledNativeArtifact,
    realize_native_artifact_with_checked_boundary_operator_scope,
};

/// Realize a receipt-coupled checked `ProgramEntry` artifact and return its
/// independently validated, owned native settlement alongside the ordinary
/// authority-free native artifact.
pub fn realize_program_entry_native_artifact(
    produced: ProducedProgramEntryTerminalArtifact,
    request: NativeRealizationRequest<'_>,
) -> Result<SettledNativeArtifact, Vec<Diagnostic>> {
    let (artifact, checked_entry, checked_scope) = produced.into_parts();
    let program_entry = validate_native_program_entry_settlement(
        &artifact,
        &checked_entry,
        request.program_entry,
        request.target,
    )
    .map_err(|error| realization_error("checked ProgramEntry settlement", error))?;
    let artifact = realize_native_artifact_with_checked_boundary_operator_scope(
        artifact,
        &checked_scope,
        request,
    )?;
    Ok(SettledNativeArtifact {
        artifact,
        program_entry,
    })
}
