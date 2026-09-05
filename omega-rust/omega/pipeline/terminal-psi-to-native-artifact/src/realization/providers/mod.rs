//! Optimizer module role: executable entrance. Provider coordination for native realization: settle external executions,
//! then admit the exact checked-provider installation retained by the plan.

mod adapters;
mod compiler_builtins;
mod installation;
mod settlements;

#[cfg(test)]
pub(crate) use adapters::project_selected_provider_adapters_for_requirements;

use crate::realization::model::{NativeRealizationCoreRequest, NativeRealizationInput};
use abstract_operations_to_target_operations::AdmittedBoundarySettlement;
use diagnostics::Diagnostic;
use native_artifact::NativeProviderExecution;
use terminal_psi_to_abstract_operations::AdmittedProviderInstallation;

pub(crate) struct AdmittedNativeProviders<'execution> {
    pub(crate) settlements: Vec<AdmittedBoundarySettlement<'execution>>,
    pub(crate) executions: Vec<NativeProviderExecution>,
    pub(crate) terminal_authority_policy_identity: effects::TerminalAuthorityPolicyIdentity,
    pub(crate) terminal_authority_permission_policy_identity:
        effects::TerminalAuthorityPermissionPolicyIdentity,
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
    request: &NativeRealizationCoreRequest<'request>,
) -> Result<AdmittedNativeProviders<'request>, Vec<Diagnostic>> {
    let (settlements, executions, mut mechanisms) =
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
    let terminal_authority_closure_review =
        crate::realization::terminal_authority_review::review_terminal_authority_closure(
            terminal_artifact_identity,
            request.program_entry.source().target_slot().owner,
            input.plan(),
            request.selected_provider_plans,
            &request.terminal_authority_policy,
            &request.terminal_authority_permission_policy,
            &mechanisms,
            installation
                .as_ref()
                .map(AdmittedProviderInstallation::installed_candidates)
                .unwrap_or_default(),
        )
        .map_err(|error| {
            crate::realization::diagnostics::realization_error(
                "terminal-authority closure review",
                error,
            )
        })?;
    Ok(AdmittedNativeProviders {
        settlements,
        executions,
        terminal_authority_policy_identity: request.terminal_authority_policy.identity(),
        terminal_authority_permission_policy_identity: request
            .terminal_authority_permission_policy
            .identity(),
        terminal_authority_closure_review,
        installation,
    })
}
