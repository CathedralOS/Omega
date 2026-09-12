//! Checked ProgramEntry settlement before ordinary native realization.

use diagnostics::Diagnostic;
use terminal_production::ProducedProgramEntryTerminalArtifact;

use crate::entry_settlement::validate_native_program_entry_settlement;

use super::diagnostics::realization_error;
use super::{
    NativeRealizationRequest, RequestedNativeArtifactError, SettledNativeArtifact,
    realize_native_artifact,
};

/// Realize a receipt-coupled checked `ProgramEntry` artifact and return its
/// independently validated, owned native settlement alongside the ordinary
/// authority-free native artifact.
pub fn realize_program_entry_native_artifact(
    produced: ProducedProgramEntryTerminalArtifact,
    request: NativeRealizationRequest<'_>,
) -> Result<SettledNativeArtifact, RequestedNativeArtifactError> {
    let (
        artifact,
        checked_entry,
        checked_scope,
        selected_ieee_float_fma_occurrences,
        selected_ieee_float_comparison_occurrences,
    ) = produced.into_parts();
    let validate_entry = || -> Result<_, Vec<Diagnostic>> {
        if let Some(scope) = request.checked_scope {
            scope
                .validate_for_artifact(&artifact)
                .map_err(|error| realization_error("checked boundary-operator scope", error))?;
        }
        if !selected_ieee_float_comparison_occurrences.is_empty() {
            return Err(vec![Diagnostic::error(
                "ProgramEntry native realization does not consume IEEE comparison occurrence custody",
            )]);
        }
        if !selected_ieee_float_fma_occurrences.is_empty() {
            return Err(vec![Diagnostic::error(
                "receipt-coupled ProgramEntry realization does not yet consume retained IEEE-FMA occurrence custody",
            )]);
        }
        let program_entry = validate_native_program_entry_settlement(
            &artifact,
            &checked_entry,
            request.program_entry,
            request.target,
        )
        .map_err(|error| realization_error("checked ProgramEntry settlement", error))?;
        Ok(program_entry)
    };
    let program_entry = match validate_entry() {
        Ok(entry) => entry,
        Err(diagnostics) => {
            return Err(RequestedNativeArtifactError {
                image_request: request.image_request,
                diagnostics,
            });
        }
    };
    let artifact = realize_native_artifact(
        artifact,
        NativeRealizationRequest {
            checked_scope: Some(&checked_scope),
            ..request
        },
    )?;
    Ok(SettledNativeArtifact {
        artifact,
        program_entry,
    })
}
