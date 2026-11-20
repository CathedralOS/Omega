//! A completed provider selection stays with the authorized program and scope that issued it.
use crate::{NativeExecutionScope, TerminalNativeRealizationProposal};
use diagnostics::Diagnostic;
use selected_provider_plans::SelectedProviderPlanFacts;

pub struct ScopedProviderSelection {
    authorization: psi_authorization::AuthorizedTerminalArtifact,
    scope: NativeExecutionScope,
    selection: SelectedProviderPlanFacts,
}

impl ScopedProviderSelection {
    /// Attach only the direct provider selection checked for these exact inputs.
    pub fn bind(
        authorization: &psi_authorization::AuthorizedTerminalArtifact,
        proposal: &TerminalNativeRealizationProposal,
        mut scope: NativeExecutionScope,
        verified: direct_boundary_bindings::VerifiedDirectProviderSelection<'_>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let artifact = authorization.artifact();
        scope.require_proposal_context(artifact.view(), proposal)?;
        let selection = verified
            .into_selection(
                artifact.semantic_module(),
                &proposal.providers().selection,
                &proposal.providers().direct_boundary_leaves,
                scope.emitted_reach(),
            )
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        scope.bind_completed_provider_selection(&selection);
        Ok(Self {
            authorization: authorization.clone(),
            scope,
            selection,
        })
    }

    pub const fn authorization(&self) -> &psi_authorization::AuthorizedTerminalArtifact {
        &self.authorization
    }
    pub const fn scope(&self) -> &NativeExecutionScope {
        &self.scope
    }
    pub const fn selection(&self) -> &SelectedProviderPlanFacts {
        &self.selection
    }
}
