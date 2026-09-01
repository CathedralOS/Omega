//! Optimizer module role: executable entrance. Validate custody, admit providers, emit, and replay.

mod boundary_applications;
mod callback_custody;
mod diagnostics;
mod input;
mod machine_code;
mod model;
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
pub use model::{
    NativeBoundaryRealization, NativeCallbackThunkSettlement, NativeCompilerBuiltinSettlement,
    NativeProviderSettlement, NativeRealizationRequest, SettledNativeArtifact,
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

use boundary_applications::retain_boundary_application_coverage;
use diagnostics::realization_error;
use input::lower_realization_input;
use machine_code::emit_realization_machine_code;
use omega_native_artifact::NativeArtifact;
use output::assemble_native_artifact;
use providers::{AdmittedNativeProviders, admit_native_providers};
use psi_diagnostics::Diagnostic;

/// Realize a canonical Terminal-Psi artifact into an authority-free object and
/// image while retaining source-entry settlement for every compilation route.
pub fn realize_native_artifact(
    artifact: psi_terminal_codec::CanonicalTerminalArtifact,
    request: NativeRealizationRequest<'_>,
) -> Result<NativeArtifact, Vec<Diagnostic>> {
    realize_native_artifact_with_optional_checked_scope(artifact, None, request)
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
    let boundary_application_coverage = retain_boundary_application_coverage(
        &artifact,
        checked_scope,
        request.boundary_application_coverage,
    )?;
    let semantic_bytes = artifact.semantic_bytes();
    let proof_bytes = artifact.proof_bytes();
    let terminal_artifact_identity = *artifact.manifest().identity().as_bytes();
    let input = lower_realization_input(semantic_bytes, proof_bytes, &request)?;
    let AdmittedNativeProviders {
        settlements,
        executions,
        terminal_authority_policy_identity,
        terminal_authority_permission_policy_identity,
        terminal_authority_closure_review,
        installation,
    } = admit_native_providers(
        &input,
        semantic_bytes,
        proof_bytes,
        terminal_artifact_identity,
        &request,
    )?;
    let physical_evidence_scope = input.physical_evidence_scope(checked_scope);
    let machine_code = emit_realization_machine_code(input, installation, &settlements, &request)?;
    assemble_native_artifact(
        artifact,
        &machine_code,
        executions,
        terminal_authority_policy_identity,
        terminal_authority_permission_policy_identity,
        terminal_authority_closure_review,
        boundary_application_coverage,
        physical_evidence_scope,
        &request,
    )
}
