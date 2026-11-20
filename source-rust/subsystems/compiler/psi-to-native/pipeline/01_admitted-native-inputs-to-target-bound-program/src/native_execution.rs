//! Issue the target-selected reach before the driver requests scalar values or lowers a body.
use diagnostics::Diagnostic;
use native_execution_scope::derive_native_scope;
use verified_optimization_input::{
    AdmittedNativeComponentCall, NativeExecutionFacts, NativeExecutionScope, ProposalScopeRequest,
    TerminalNativeRealizationProposal,
};

mod provider_bindings;

/// A native program or export whose reachable provider bindings stage 01 completed.
pub struct PreparedNativeExecution(NativeExecutionFacts);

impl std::ops::Deref for PreparedNativeExecution {
    type Target = NativeExecutionFacts;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl PreparedNativeExecution {
    pub fn with_operation_evidence(
        self,
        artifact: &terminal_codec::CanonicalTerminalArtifact,
        evidence: Option<std::sync::Arc<terminal_codec::EvidenceSeals>>,
    ) -> Result<Self, Vec<Diagnostic>> {
        self.0.with_operation_evidence(artifact, evidence).map(Self)
    }
}

fn issue_scope(request: ProposalScopeRequest<'_>) -> Result<NativeExecutionScope, Vec<Diagnostic>> {
    derive_native_scope(request).map_err(|reason| vec![Diagnostic::error(reason.to_string())])
}

pub fn prepare_program(
    proposal: &TerminalNativeRealizationProposal,
    authorization: &psi_authorization::AuthorizedTerminalArtifact,
    components: &[&AdmittedNativeComponentCall<'_>],
) -> Result<PreparedNativeExecution, Vec<Diagnostic>> {
    let request = proposal.program_scope_request(authorization.artifact(), components)?;
    provider_bindings::bind(proposal, authorization, issue_scope(request)?)
}

pub fn prepare_export(
    proposal: &TerminalNativeRealizationProposal,
    authorization: &psi_authorization::AuthorizedTerminalArtifact,
    entry: terminal_psi::TargetMachineCallable,
) -> Result<PreparedNativeExecution, Vec<Diagnostic>> {
    let request = proposal.callable_scope_request(authorization.artifact(), entry, None)?;
    provider_bindings::bind(proposal, authorization, issue_scope(request)?)
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
    coverage: boundary_applications::TerminalBoundaryApplicationCoverage,
}

impl<'a> BoundNativeCallable<'a> {
    pub fn prepare(
        proposal: &'a TerminalNativeRealizationProposal,
        execution: &'a PreparedNativeExecution,
        artifact: &'a terminal_codec::CanonicalTerminalArtifact,
        entry: terminal_psi::TargetMachineCallable,
    ) -> Result<Self, Vec<Diagnostic>> {
        let request = proposal.callable_scope_request(
            artifact,
            entry,
            Some(execution.checked_providers()),
        )?;
        let scope = issue_scope(request)?;
        let coverage = provider_bindings::callable_coverage(
            proposal,
            artifact,
            &scope,
            execution.selection(),
        )?;
        Ok(Self {
            proposal,
            execution,
            artifact,
            entry,
            scope,
            coverage,
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
        boundary_applications::TerminalBoundaryApplicationCoverage,
    ) {
        (
            self.proposal,
            self.execution,
            self.artifact,
            self.entry,
            self.scope,
            self.coverage,
        )
    }
}
