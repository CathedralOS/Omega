//! Optimizer module role: executable entrance. Provider coordination for native realization: settle external executions,
//! then admit the exact checked-provider installation retained by the plan.

mod adapters;
mod compiler_builtins;
mod installation;
mod settlements;

#[cfg(test)]
pub(crate) use adapters::project_selected_provider_adapters_for_requirements;

use crate::native_realization::realization_request::{
    NativeRealizationInput, NativeRealizationRequest,
};
use abstract_operations_to_target_operations::AdmittedBoundarySettlement;
use diagnostics::Diagnostic;
use native_artifact::NativeProviderExecution;
use terminal_psi_to_abstract_operations::AdmittedProviderInstallation;

pub(crate) struct AdmittedNativeProviders<'execution> {
    pub(crate) settlements: Vec<AdmittedBoundarySettlement<'execution>>,
    pub(crate) executions: Vec<NativeProviderExecution>,
    pub(crate) terminal_authority_policy_identity: effects::TerminalAuthorityPolicyIdentity,
    /// `Some` only when a receiving permission policy was explicitly supplied;
    /// `None` means the artifact makes no receiver-admission claim.
    pub(crate) terminal_authority_permission_policy_identity:
        Option<effects::TerminalAuthorityPermissionPolicyIdentity>,
    pub(crate) terminal_authority_closure_review: effects::TerminalAuthorityClosureReviewReceipt,
    pub(crate) installation: Option<AdmittedProviderInstallation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AdmittedTerminalMechanism {
    pub(crate) boundary: semantic_vocabulary::BoundaryMachineId,
    pub(crate) mechanism: effects::TerminalMechanismIdentity,
}

pub(crate) fn admit_native_providers<'request>(
    input: &NativeRealizationInput,
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    terminal_artifact_identity: [u8; 32],
    request: &NativeRealizationRequest<'request>,
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
            crate::native_realization::terminal_authority_policy::terminal_authority_policy_with_rows(
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
        .unwrap_or(&request.terminal_authority_policy);
    let terminal_authority_closure_review =
        crate::native_realization::terminal_authority_review::review_terminal_authority_closure(
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
            crate::native_realization::realization_diagnostics::realization_error(
                "terminal-authority closure review",
                error,
            )
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
