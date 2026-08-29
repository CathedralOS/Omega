//! Provider coordination for native realization: settle external executions,
//! then admit the exact checked-provider installation retained by the plan.

mod adapters;
mod installation;
mod settlements;

#[cfg(test)]
pub(crate) use adapters::project_selected_provider_adapters_for_requirements;

use crate::realization::model::{NativeRealizationInput, NativeRealizationRequest};
use omega_abstract_operations_to_target_operations::AdmittedBoundarySettlement;
use omega_native_artifact::NativeProviderExecution;
use omega_psi_to_abstract_operations::AdmittedProviderInstallation;
use psi_diagnostics::Diagnostic;

pub(crate) struct AdmittedNativeProviders<'execution> {
    pub(crate) settlements: Vec<AdmittedBoundarySettlement<'execution>>,
    pub(crate) executions: Vec<NativeProviderExecution>,
    pub(crate) installation: Option<AdmittedProviderInstallation>,
}

pub(crate) fn admit_native_providers<'request>(
    input: &NativeRealizationInput,
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    request: &NativeRealizationRequest<'request>,
) -> Result<AdmittedNativeProviders<'request>, Vec<Diagnostic>> {
    let (settlements, executions) = settlements::settle_provider_executions(input, request)?;
    let installation = installation::admit_checked_provider_installation(
        input,
        semantic_bytes,
        proof_bytes,
        request,
    )?;
    Ok(AdmittedNativeProviders {
        settlements,
        executions,
        installation,
    })
}
