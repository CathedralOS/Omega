//! Issue the target-selected reach before the driver requests scalar values or lowers a body.
use diagnostics::Diagnostic;
use native_execution_scope::{NativeScopeEntry, derive_native_scope};
use verified_optimization_input::{
    NativeExecutionFacts, NativeExecutionScope,
    TerminalNativeRealizationProposal,
};

mod provider_bindings;

/// A native program whose reachable provider bindings stage 01 completed.
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

pub fn prepare_program(
    proposal: &TerminalNativeRealizationProposal,
    authorization: &psi_authorization::AuthorizedTerminalArtifact,
) -> Result<PreparedNativeExecution, Vec<Diagnostic>> {
    let scope = derive_native_scope(authorization.artifact(), proposal, NativeScopeEntry::Program)?;
    provider_bindings::bind(proposal, authorization, scope)
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
        let scope = derive_native_scope(
            artifact,
            proposal,
            NativeScopeEntry::Callable {
                entry,
                parent: execution.checked_providers(),
            },
        )?;
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
