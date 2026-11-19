//! Admit each component input and select the scalar values and stage 01 execution scope for its exports.

use diagnostics::Diagnostic;
use provider_admission::TerminalAuthorityPermissionPolicy;
use provider_plans::ProviderPlan;
use terminal_psi::TerminalIdentityTable;
use verified_optimization_input::{
    AdmittedNativeComponentCall, ComponentNativeInputs, PreparedNativeExecution,
};

pub struct ComponentCompilation<'input> {
    pub input: ComponentNativeInputs<'input>,
    pub plan: &'input ProviderPlan,
    pub receiving_identities: &'input TerminalIdentityTable,
    pub accepted_package_permissions: &'input TerminalAuthorityPermissionPolicy,
    /// Completed scalar values read by the exports.
    pub scalars: Option<&'input psi_scalar_applications::AdmittedScalarApplications>,
}

pub(crate) struct PreparedComponent<'input> {
    pub(crate) compilation: &'input ComponentCompilation<'input>,
    pub(crate) calls: Vec<AdmittedNativeComponentCall<'input>>,
    pub(crate) scalars: Option<&'input psi_scalar_applications::AdmittedScalarApplications>,
    pub(crate) exports: Vec<PreparedNativeExecution>,
}

pub(crate) fn prepare_components<'input>(
    components: &'input [ComponentCompilation<'input>],
    receiver: terminal_codec::TerminalArtifactView<'_>,
) -> Result<Vec<PreparedComponent<'input>>, Vec<Diagnostic>> {
    let mut prepared = Vec::with_capacity(components.len());
    for compilation in components {
        let input = &compilation.input;
        let calls = input
            .admit_native_calls(receiver)
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        let artifact = input.component().shared_artifact();
        let proposal = input.proposal();
        let exports = input
            .exports()
            .iter()
            .map(|export| {
                provider_realization::native_execution::prepare_export(
                        proposal,
                        input.authorization(),
                        terminal_psi::TargetMachineCallable::Machine(export.candidate.candidate),
                    )?
                    .with_operation_evidence(artifact, input.evidence().cloned())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut demand = Vec::new();
        for execution in &exports {
            demand.extend(execution.scalar_application_demand(proposal.target_profile())?);
        }
        let scalars = crate::target_inputs::required_scalars::required_scalars(
            artifact,
            &demand,
            compilation.scalars,
        )?;
        prepared.push(PreparedComponent {
            compilation,
            calls,
            scalars,
            exports,
        });
    }
    Ok(prepared)
}
