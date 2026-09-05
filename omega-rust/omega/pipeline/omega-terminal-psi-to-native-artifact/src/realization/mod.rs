//! Optimizer module role: executable entrance. Validate custody, admit providers, emit, and replay.

mod api;
mod boundary_applications;
mod callback_custody;
mod callback_machine_code;
mod diagnostics;
mod input;
mod machine_code;
mod model;
mod native_artifact;
mod optimization_stage;
mod optimized_fragment_projection;
mod optimized_fragment_unit_stack;
mod output;
mod physical_stage;
mod program_entry;
pub(crate) mod providers;
mod target_stage;
mod terminal_authority_permission_policy;
mod terminal_authority_policy;
mod terminal_authority_review;

pub use api::{
    realize_native_artifact_with_checked_boundary_operator_scope,
    realize_native_artifact_with_checked_boundary_operator_scope_and_prepared_input,
    realize_requested_native_artifact,
    realize_requested_native_artifact_with_checked_boundary_operator_scope,
};
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

/// Realize a canonical Terminal-Psi artifact into an authority-free object and
/// image while retaining source-entry settlement for every compilation route.
pub fn realize_native_artifact(
    artifact: psi_terminal_codec::CanonicalTerminalArtifact,
    request: NativeRealizationRequest<'_>,
) -> Result<omega_native_artifact::NativeArtifact, Vec<psi_diagnostics::Diagnostic>> {
    native_artifact::realize(artifact, None, request, None)
}
