//! Public native-realization entrypoints.

use super::diagnostics::realization_error;
use super::input::PreparedNativeRealizationInput;
use super::model::{
    NativeRealizationRequest, RequestedNativeArtifact, RequestedNativeArtifactError,
    RequestedNativeRealizationRequest,
};
use omega_native_artifact::NativeArtifact;
use psi_diagnostics::Diagnostic;

/// Realize a canonical Terminal artifact through the writer selected by the
/// exact object contents and the authority-distinct image request.
pub fn realize_requested_native_artifact(
    artifact: psi_terminal_codec::CanonicalTerminalArtifact,
    request: RequestedNativeRealizationRequest<'_>,
) -> Result<RequestedNativeArtifact, RequestedNativeArtifactError> {
    super::native_artifact::realize_requested(artifact, None, request, None)
}

/// Requested realization retaining the exact checked D29 scope produced with
/// the same Terminal artifact.
pub fn realize_requested_native_artifact_with_checked_boundary_operator_scope(
    artifact: psi_terminal_codec::CanonicalTerminalArtifact,
    checked_scope: &psi_checked_trees_to_terminal::CheckedBoundaryOperatorApplicationScope,
    request: RequestedNativeRealizationRequest<'_>,
) -> Result<RequestedNativeArtifact, RequestedNativeArtifactError> {
    if let Err(error) = checked_scope.validate_for_artifact(&artifact) {
        return Err(RequestedNativeArtifactError {
            image_request: request.image_request,
            diagnostics: realization_error("checked boundary-operator scope", error),
        });
    }
    super::native_artifact::realize_requested(artifact, Some(checked_scope), request, None)
}

/// Realize an artifact while retaining the exact checked D29 scope emitted by
/// the same Terminal production; callers cannot substitute a count or flag.
pub fn realize_native_artifact_with_checked_boundary_operator_scope(
    artifact: psi_terminal_codec::CanonicalTerminalArtifact,
    checked_scope: &psi_checked_trees_to_terminal::CheckedBoundaryOperatorApplicationScope,
    request: NativeRealizationRequest<'_>,
) -> Result<NativeArtifact, Vec<Diagnostic>> {
    checked_scope
        .validate_for_artifact(&artifact)
        .map_err(|error| realization_error("checked boundary-operator scope", error))?;
    super::native_artifact::realize(artifact, Some(checked_scope), request, None)
}

/// Realize one exact target child from a previously verified target-neutral
/// Terminal input. The prepared carrier is reopened only after exact artifact,
/// admission-profile, and optimization-entrance equality is rechecked.
pub fn realize_native_artifact_with_checked_boundary_operator_scope_and_prepared_input(
    artifact: psi_terminal_codec::CanonicalTerminalArtifact,
    checked_scope: &psi_checked_trees_to_terminal::CheckedBoundaryOperatorApplicationScope,
    request: NativeRealizationRequest<'_>,
    prepared_input: &PreparedNativeRealizationInput,
) -> Result<NativeArtifact, Vec<Diagnostic>> {
    checked_scope
        .validate_for_artifact(&artifact)
        .map_err(|error| realization_error("checked boundary-operator scope", error))?;
    super::native_artifact::realize(artifact, Some(checked_scope), request, Some(prepared_input))
}
