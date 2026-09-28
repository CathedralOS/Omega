//! Native publication: validate Terminal custody, admit providers, emit, and replay.
//!
//! The request supplies realization authority separately from the portable
//! program, including the retained behavior-exclusion union; selected
//! optimizations do not choose a different publication route.

mod callback_thunks;
mod object_emission;
pub(crate) mod realization_diagnostics;
mod realization_request;
pub use terminal_psi_to_abstract_operations::provider_admission::terminal_authority_permissions;

pub use realization_request::{
    NativeBoundaryRealization, NativeCallbackThunkSettlement, NativeCompilerBuiltinSettlement,
    NativeProviderSettlement, NativeRealizationRequest, RequestedNativeArtifact,
    RequestedNativeArtifactError,
};
pub use terminal_psi_to_abstract_operations::provider_admission::{
    COMPILER_INTRINSIC_TERMINAL_AUTHORITY_POLICY_VERSION, CompilerIntrinsicTerminalAuthorityPolicy,
    FilesystemCohortDisposition, FilesystemOrdinaryReleaseContract,
    TERMINAL_AUTHORITY_POLICY_VERSION, TerminalAuthorityPolicy, TerminalAuthorityPolicyBuildError,
    TerminalAuthorityPolicyRow, UnclassifiedCompilerIntrinsicTerminalMechanism,
    UnclassifiedTerminalMechanism, UnsettledFilesystemRequirement, UnsettledTimeHostRequirement,
    conservative_syscall_terminal_mechanism, current_compiler_intrinsic_terminal_authority_policy,
    current_terminal_authority_policy, filesystem_host_permission_row,
    filesystem_host_permission_rows, filesystem_mechanism_row,
    filesystem_ordinary_release_contract, filesystem_release_bound_mechanism,
    filesystem_release_mechanism_row, normalized_foreign_terminal_mechanism,
    normalized_foreign_terminal_mechanism_with_callback_materializations,
    settled_filesystem_cohort, settled_time_host_cohort, terminal_authority_policy_with_rows,
    time_host_mechanism_row, time_host_permission_row, time_host_permission_rows,
};
pub use terminal_psi_to_abstract_operations::provider_admission::{
    MissingTerminalAuthorityPermission, TERMINAL_AUTHORITY_PERMISSION_POLICY_VERSION,
    TerminalAuthorityPermissionPolicy, TerminalAuthorityPermissionPolicyBuildError,
    TerminalAuthorityPermissionPolicyRow, current_terminal_authority_permission_policy,
    terminal_authority_permission_policy_with_rows,
};
pub use terminal_psi_to_abstract_operations::{
    PreparedNativeRealizationInput, prepare_native_realization_input,
};

use diagnostics::Diagnostic;

use self::object_emission::emit_realization_object;
use native_artifact::{
    NativeArtifactEmissionParts, NativeSelectedProviderClosureDigest, NativeSelectedProviderPlan,
};
use terminal_psi_to_abstract_operations::provider_admission::AdmittedNativeProviders;
use terminal_psi_to_abstract_operations::{AdmittedNativeRealization, admit_native_realization};

/// Product-owned per-pass ceilings shared by program and callback realization.
/// Identity staging meters input traversal as well as optimizer iterations.
fn native_optimization_budget() -> optimization_core::OptimizationWorkBudget {
    optimization_core::OptimizationWorkBudget::new(1_000_000, 100_000, 100_000, 100_000, 100_000)
        .expect("compiler baseline optimizer ceilings are nonzero")
}

/// Realize one Terminal artifact using explicit image, custody, reuse, and
/// behavior-exclusion inputs. The request's retained behavior-exclusion union
/// is adjudicated against the mechanism-closure review by stage 00,
/// so a requested absence holds whether or not the request carries a receiver
/// permission policy, and a violated exclusion publishes no product. Failure
/// returns the exact image request; no product is silently substituted.
pub fn realize_native_artifact(
    artifact: terminal_codec::CanonicalTerminalArtifact,
    request: NativeRealizationRequest<'_>,
) -> Result<RequestedNativeArtifact, RequestedNativeArtifactError> {
    realize_image(artifact, &request).map_err(|diagnostics| RequestedNativeArtifactError {
        image_request: request.image_request,
        diagnostics,
    })
}

fn realize_image(
    artifact: terminal_codec::CanonicalTerminalArtifact,
    request: &NativeRealizationRequest<'_>,
) -> Result<RequestedNativeArtifact, Vec<Diagnostic>> {
    let AdmittedNativeRealization {
        input,
        providers,
        boundary_application_coverage,
    } = admit_native_realization(&artifact, &request.admission)?;
    let receiver_settlement = native_artifact::admit_executable_entry_receiver(
        &input,
        &artifact,
        request.admission.providers.program_entry,
        request.admission.providers.target,
    )?;
    let AdmittedNativeProviders {
        settlements,
        executions,
        terminal_authority_policy_identity,
        terminal_authority_permission_policy_identity,
        terminal_authority_closure_review,
        installation,
    } = providers;
    let executions = executions
        .into_iter()
        .map(native_artifact::NativeProviderExecution::from_evidence)
        .collect();
    let emitted = emit_realization_object(
        input,
        installation,
        &settlements,
        boundary_application_coverage.as_ref(),
        receiver_settlement.as_ref(),
        &artifact,
        request,
    )?;
    RequestedNativeArtifact::emit(NativeArtifactEmissionParts {
        target: request.admission.providers.target,
        psi_artifact: artifact,
        object: emitted.object,
        image: request.image_request.clone(),
        selected_provider_closure_report_identity: request
            .admission
            .providers
            .selected_provider_plans
            .compatibility_report_identity(),
        selected_provider_closure_digest: NativeSelectedProviderClosureDigest::from_digest(
            *request
                .admission
                .providers
                .selected_provider_plans
                .identity_digest()
                .as_bytes(),
        ),
        selected_provider_plans: NativeSelectedProviderPlan::project_selection(
            request.admission.providers.selected_provider_plans,
        ),
        provider_executions: executions,
        terminal_authority_policy_identity,
        terminal_authority_permission_policy_identity,
        terminal_authority_closure_review,
        boundary_application_coverage,
        physical_evidence_scope: emitted.physical_evidence_scope,
    })
}
