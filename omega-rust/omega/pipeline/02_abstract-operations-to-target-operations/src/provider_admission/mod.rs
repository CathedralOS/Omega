//! Optimizer module role: executable entrance. Provider coordination for native realization: settle external executions,
//! then admit the exact checked-provider installation retained by the plan.

mod adapters;
mod compiler_builtins;
mod diagnostics;
mod installation;
mod request;
mod settlements;
mod terminal_authority_permission_policy;
pub mod terminal_authority_permissions;
mod terminal_authority_policy;
mod terminal_authority_review;

pub use request::{
    NativeBoundaryRealization, NativeCompilerBuiltinSettlement, NativeProviderSettlement,
    ProviderAdmissionRequest,
};
pub use terminal_authority_permission_policy::{
    MissingTerminalAuthorityPermission, TERMINAL_AUTHORITY_PERMISSION_POLICY_VERSION,
    TerminalAuthorityPermissionPolicy, TerminalAuthorityPermissionPolicyBuildError,
    TerminalAuthorityPermissionPolicyRow, current_terminal_authority_permission_policy,
    terminal_authority_permission_policy_with_rows,
};
pub use terminal_authority_policy::{
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

use crate::AdmittedBoundarySettlement;
use ::diagnostics::Diagnostic;
use installation_evidence::ProviderExecutionEvidence;
use terminal_psi_to_abstract_operations::AdmittedProviderInstallation;
use terminal_psi_to_abstract_operations::VerifiedNativeArtifactInput as NativeRealizationInput;

pub struct AdmittedNativeProviders<'execution> {
    pub settlements: Vec<AdmittedBoundarySettlement<'execution>>,
    pub executions: Vec<&'execution dyn ProviderExecutionEvidence>,
    pub terminal_authority_policy_identity: effects::TerminalAuthorityPolicyIdentity,
    /// `Some` only when a receiving permission policy was explicitly supplied;
    /// `None` means the artifact makes no receiver-admission claim.
    pub terminal_authority_permission_policy_identity:
        Option<effects::TerminalAuthorityPermissionPolicyIdentity>,
    pub terminal_authority_closure_review: effects::TerminalAuthorityClosureReviewReceipt,
    pub installation: Option<AdmittedProviderInstallation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdmittedTerminalMechanism {
    pub boundary: semantic_vocabulary::BoundaryMachineId,
    pub mechanism: effects::TerminalMechanismIdentity,
}

pub fn admit_native_providers<'request>(
    input: &NativeRealizationInput,
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    terminal_artifact_identity: [u8; 32],
    request: &ProviderAdmissionRequest<'request>,
) -> Result<AdmittedNativeProviders<'request>, Vec<Diagnostic>> {
    let (settlements, executions, mut mechanisms, cohort_rows) =
        settlements::settle_provider_executions(input, request)?;
    let mut settlements = settlements;
    let (builtin_settlements, builtin_mechanisms) =
        compiler_builtins::settle_compiler_builtins(input, request)?;
    settlements.extend(builtin_settlements);
    mechanisms.extend(builtin_mechanisms);
    let installation = installation::admit_checked_provider_installation(
        input,
        semantic_bytes,
        proof_bytes,
        request,
    )?;
    // The toolchain-settled `FilesystemHost` cohort rows emitted for demanded
    // leaves join the supplied receiving rows into the effective policy the
    // closure review and the artifact identity bind. A byte-identical supplied
    // row dedups; a supplied row for the same mechanism with a different
    // disposition is a forged classification and rejects as a duplicate key.
    let effective_policy = if cohort_rows.is_empty() {
        None
    } else {
        let mut rows = request.terminal_authority_policy.explicit_rows().to_vec();
        for row in cohort_rows {
            if !rows.contains(&row) {
                rows.push(row);
            }
        }
        Some(
            terminal_authority_policy::terminal_authority_policy_with_rows(
                rows,
            )
            .map_err(|error| {
                vec![Diagnostic::error(format!(
                    "settled filesystem cohort mechanism rows do not merge into the receiving terminal-authority policy without substitution: {error:?}"
                ))]
            })?,
        )
    };
    let effective_policy = effective_policy
        .as_ref()
        .unwrap_or(request.terminal_authority_policy);
    let terminal_authority_closure_review =
        terminal_authority_review::review_terminal_authority_closure(
            terminal_artifact_identity,
            request.program_entry.source().target_slot().owner,
            input.plan(),
            request.selected_provider_plans,
            effective_policy,
            request.terminal_authority_permission_policy.as_ref(),
            &mechanisms,
            installation
                .as_ref()
                .map(AdmittedProviderInstallation::installed_candidates)
                .unwrap_or_default(),
        )
        .map_err(|error| {
            diagnostics::realization_error("terminal-authority closure review", error)
        })?;
    Ok(AdmittedNativeProviders {
        settlements,
        executions,
        terminal_authority_policy_identity: effective_policy.identity(),
        terminal_authority_permission_policy_identity: request
            .terminal_authority_permission_policy
            .as_ref()
            .map(|policy| policy.identity()),
        terminal_authority_closure_review,
        installation,
    })
}
