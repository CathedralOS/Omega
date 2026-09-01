//! Optimizer module role: executable entrance. Native-realization entrance: validate source custody, choose the exact
//! ordinary or selected path, admit providers, emit machine code, and replay
//! the authority-free artifact.

mod callback_custody;
mod diagnostics;
mod input;
mod machine_code;
mod model;
mod output;
mod providers;
mod terminal_authority_policy;

pub use callback_custody::{
    CallbackCustodyNativeRealizationError, RealizedNativeArtifactWithCallbackCustody,
    realize_native_artifact_with_callback_custody,
};
pub use model::{
    NativeBoundaryRealization, NativeCompilerBuiltinSettlement, NativeProviderSettlement,
    NativeRealizationRequest, SettledNativeArtifact,
};
pub use terminal_authority_policy::{
    COMPILER_INTRINSIC_TERMINAL_AUTHORITY_POLICY_VERSION, CompilerIntrinsicTerminalAuthorityPolicy,
    UnclassifiedCompilerIntrinsicTerminalMechanism,
    current_compiler_intrinsic_terminal_authority_policy,
};

use crate::entry_settlement::validate_native_program_entry_settlement;
use diagnostics::realization_error;
use input::lower_realization_input;
use machine_code::emit_realization_machine_code;
use omega_native_artifact::NativeArtifact;
use output::assemble_native_artifact;
use providers::{AdmittedNativeProviders, admit_native_providers};
use psi_checked_trees_to_terminal::ProducedProgramEntryTerminalArtifact;
use psi_diagnostics::Diagnostic;

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

/// Realize one canonical Terminal-Psi artifact into an authority-free target
/// object and executable image while retaining its captured source-entry
/// settlement. Ordinary native compilation and component packaging share this
/// exact operation.
pub fn realize_native_artifact(
    artifact: psi_terminal_codec::CanonicalTerminalArtifact,
    request: NativeRealizationRequest<'_>,
) -> Result<NativeArtifact, Vec<Diagnostic>> {
    realize_native_artifact_with_optional_checked_scope(artifact, None, request)
}

/// Realize an artifact while retaining the exact checked D29 scope emitted by
/// the same canonical Terminal production. Generic artifact callers cannot
/// replace this receipt with a count or Boolean assertion.
pub fn realize_native_artifact_with_checked_boundary_operator_scope(
    artifact: psi_terminal_codec::CanonicalTerminalArtifact,
    checked_scope: &psi_checked_trees_to_terminal::CheckedBoundaryOperatorApplicationScope,
    request: NativeRealizationRequest<'_>,
) -> Result<NativeArtifact, Vec<Diagnostic>> {
    checked_scope
        .validate_for_artifact(&artifact)
        .map_err(|error| realization_error("checked boundary-operator scope", error))?;
    realize_native_artifact_with_optional_checked_scope(artifact, Some(checked_scope), request)
}

fn realize_native_artifact_with_optional_checked_scope(
    artifact: psi_terminal_codec::CanonicalTerminalArtifact,
    checked_scope: Option<&psi_checked_trees_to_terminal::CheckedBoundaryOperatorApplicationScope>,
    request: NativeRealizationRequest<'_>,
) -> Result<NativeArtifact, Vec<Diagnostic>> {
    request
        .program_entry
        .validate_for_target(request.target)
        .map_err(|error| realization_error("ProgramEntry custody", error))?;
    artifact
        .validate()
        .map_err(|error| realization_error("canonical artifact replay", error))?;
    let semantic_bytes = artifact.semantic_bytes();
    let proof_bytes = artifact.proof_bytes();
    let input = lower_realization_input(semantic_bytes, proof_bytes, &request)?;
    let AdmittedNativeProviders {
        settlements,
        executions,
        installation,
    } = admit_native_providers(&input, semantic_bytes, proof_bytes, &request)?;
    let physical_evidence_scope = input.physical_evidence_scope(checked_scope);
    let machine_code = emit_realization_machine_code(input, installation, &settlements, &request)?;
    assemble_native_artifact(
        artifact,
        &machine_code,
        executions,
        physical_evidence_scope,
        &request,
    )
}

#[cfg(test)]
pub(crate) use providers::project_selected_provider_adapters_for_requirements;
