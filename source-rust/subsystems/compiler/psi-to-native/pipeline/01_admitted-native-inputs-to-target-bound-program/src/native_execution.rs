//! Issue the target-selected reach before the driver requests scalar values or lowers a body.
use diagnostics::Diagnostic;
use native_execution_scope::derive_native_scope;
use verified_optimization_input::{
    AdmittedNativeComponentCall, NativeExecutionScope, PreparedNativeExecution,
    ProposalScopeRequest, TerminalNativeRealizationProposal,
};

fn issue_scope(request: ProposalScopeRequest<'_>) -> Result<NativeExecutionScope, Vec<Diagnostic>> {
    derive_native_scope(request).map_err(|reason| vec![Diagnostic::error(reason.to_string())])
}

pub fn prepare_program(
    proposal: &TerminalNativeRealizationProposal,
    authorization: &psi_authorization::AuthorizedTerminalArtifact,
    components: &[&AdmittedNativeComponentCall<'_>],
) -> Result<PreparedNativeExecution, Vec<Diagnostic>> {
    let request = proposal.program_scope_request(authorization.artifact(), components)?;
    proposal.prepare_native_provider_demand(authorization, issue_scope(request)?)
}

pub fn prepare_export(
    proposal: &TerminalNativeRealizationProposal,
    authorization: &psi_authorization::AuthorizedTerminalArtifact,
    entry: terminal_psi::TargetMachineCallable,
) -> Result<PreparedNativeExecution, Vec<Diagnostic>> {
    let request = proposal.callable_scope_request(authorization.artifact(), entry, None)?;
    proposal.prepare_native_provider_demand(authorization, issue_scope(request)?)
}

pub fn entry_scalar_demand(
    proposal: &TerminalNativeRealizationProposal,
    authorization: &psi_authorization::AuthorizedTerminalArtifact,
) -> Result<Option<Vec<semantic_vocabulary::ScalarApplicationId>>, Vec<Diagnostic>> {
    let demand = prepare_program(proposal, authorization, &[])?
        .scalar_application_demand(proposal.target_profile())?;
    Ok(
        verified_optimization_input::requires_scalar_values(authorization.artifact(), &demand)
            .then_some(demand),
    )
}

/// An independently lowered callable inherits the selected providers of its owning execution.
pub struct BoundNativeCallable<'a> {
    proposal: &'a TerminalNativeRealizationProposal,
    execution: &'a PreparedNativeExecution,
    artifact: &'a terminal_codec::CanonicalTerminalArtifact,
    entry: terminal_psi::TargetMachineCallable,
    scope: NativeExecutionScope,
}

impl<'a> BoundNativeCallable<'a> {
    pub fn prepare(
        proposal: &'a TerminalNativeRealizationProposal,
        execution: &'a PreparedNativeExecution,
        artifact: &'a terminal_codec::CanonicalTerminalArtifact,
        entry: terminal_psi::TargetMachineCallable,
    ) -> Result<Self, Vec<Diagnostic>> {
        execution
            .authorization()
            .require_artifact(artifact)
            .map_err(|error| vec![Diagnostic::error(error.to_string())])?;
        let request = proposal.callable_scope_request(artifact, entry, Some(execution))?;
        Ok(Self {
            proposal,
            execution,
            artifact,
            entry,
            scope: issue_scope(request)?,
        })
    }

    pub fn into_parts(
        self,
    ) -> (
        &'a TerminalNativeRealizationProposal,
        &'a PreparedNativeExecution,
        &'a terminal_codec::CanonicalTerminalArtifact,
        terminal_psi::TargetMachineCallable,
        NativeExecutionScope,
    ) {
        (
            self.proposal,
            self.execution,
            self.artifact,
            self.entry,
            self.scope,
        )
    }
}
