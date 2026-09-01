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
    realize_native_artifact_with_callback_custody, CallbackCustodyNativeRealizationError,
    RealizedNativeArtifactWithCallbackCustody,
};
pub use model::{
    NativeBoundaryRealization, NativeCallbackThunkSettlement, NativeCompilerBuiltinSettlement,
    NativeProviderSettlement, NativeRealizationRequest, SettledNativeArtifact,
};
pub use program_entry::realize_program_entry_native_artifact;
pub use terminal_authority_permission_policy::{
    current_terminal_authority_permission_policy, terminal_authority_permission_policy_with_rows,
    MissingTerminalAuthorityPermission, TerminalAuthorityPermissionPolicy,
    TerminalAuthorityPermissionPolicyBuildError, TerminalAuthorityPermissionPolicyRow,
    TERMINAL_AUTHORITY_PERMISSION_POLICY_VERSION,
};
pub use terminal_authority_policy::{
    current_compiler_intrinsic_terminal_authority_policy, current_terminal_authority_policy,
    normalized_foreign_terminal_mechanism,
    normalized_foreign_terminal_mechanism_with_callback_materializations,
    terminal_authority_policy_with_rows, CompilerIntrinsicTerminalAuthorityPolicy,
    TerminalAuthorityPolicy, TerminalAuthorityPolicyBuildError, TerminalAuthorityPolicyRow,
    UnclassifiedCompilerIntrinsicTerminalMechanism, UnclassifiedTerminalMechanism,
    COMPILER_INTRINSIC_TERMINAL_AUTHORITY_POLICY_VERSION, TERMINAL_AUTHORITY_POLICY_VERSION,
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
    native_artifact::realize(artifact, None, request)
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
    native_artifact::realize(artifact, Some(checked_scope), request)
}
