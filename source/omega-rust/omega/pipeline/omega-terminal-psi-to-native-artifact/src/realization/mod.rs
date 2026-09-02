//! Optimizer module role: executable entrance. Validate custody, admit providers, emit, and replay.

mod boundary_applications;
mod callback_custody;
mod diagnostics;
mod input;
mod machine_code;
mod model;
mod native_artifact;
mod output;
mod program_entry;
pub(crate) mod providers;
mod terminal_authority_permission_policy;
mod terminal_authority_policy;
mod terminal_authority_review;

pub use callback_custody::{
    CallbackCustodyNativeRealizationError, RealizedNativeArtifactWithCallbackCustody,
    realize_native_artifact_with_callback_custody,
};
pub use input::{PreparedNativeRealizationInput, prepare_native_realization_input};
pub use model::{
    NativeBoundaryRealization, NativeCallbackThunkSettlement, NativeCompilerBuiltinSettlement,
    NativeProviderSettlement, NativeRealizationRequest, RequestedNativeArtifact,
    RequestedNativeArtifactError, RequestedNativeRealizationRequest, SettledNativeArtifact,
};
pub use program_entry::realize_program_entry_native_artifact;
pub use terminal_authority_permission_policy::{
    MissingTerminalAuthorityPermission, TERMINAL_AUTHORITY_PERMISSION_POLICY_VERSION,
    TerminalAuthorityPermissionPolicy, TerminalAuthorityPermissionPolicyBuildError,
    TerminalAuthorityPermissionPolicyRow, current_terminal_authority_permission_policy,
    terminal_authority_permission_policy_with_rows,
};
pub use terminal_authority_policy::{
    COMPILER_INTRINSIC_TERMINAL_AUTHORITY_POLICY_VERSION, CompilerIntrinsicTerminalAuthorityPolicy,
    TERMINAL_AUTHORITY_POLICY_VERSION, TerminalAuthorityPolicy, TerminalAuthorityPolicyBuildError,
    TerminalAuthorityPolicyRow, UnclassifiedCompilerIntrinsicTerminalMechanism,
    UnclassifiedTerminalMechanism, current_compiler_intrinsic_terminal_authority_policy,
    current_terminal_authority_policy, normalized_foreign_terminal_mechanism,
    normalized_foreign_terminal_mechanism_with_callback_materializations,
    terminal_authority_policy_with_rows,
};

use diagnostics::realization_error;
use omega_native_artifact::NativeArtifact;
use psi_diagnostics::Diagnostic;

/// Realize a canonical Terminal-Psi artifact into an authority-free object and
/// image while retaining source-entry settlement for every compilation route.
pub fn realize_native_artifact(
    artifact: psi_terminal_codec::CanonicalTerminalArtifact,
    request: NativeRealizationRequest<'_>,
) -> Result<NativeArtifact, Vec<Diagnostic>> {
    native_artifact::realize(artifact, None, request, None)
}

/// Realize a canonical Terminal artifact through the writer selected by the
/// exact object contents and the authority-distinct image request.
pub fn realize_requested_native_artifact(
    artifact: psi_terminal_codec::CanonicalTerminalArtifact,
    request: RequestedNativeRealizationRequest<'_>,
) -> Result<RequestedNativeArtifact, RequestedNativeArtifactError> {
    native_artifact::realize_requested(artifact, None, request, None)
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
    native_artifact::realize_requested(artifact, Some(checked_scope), request, None)
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
    native_artifact::realize(artifact, Some(checked_scope), request, None)
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
    native_artifact::realize(artifact, Some(checked_scope), request, Some(prepared_input))
}
