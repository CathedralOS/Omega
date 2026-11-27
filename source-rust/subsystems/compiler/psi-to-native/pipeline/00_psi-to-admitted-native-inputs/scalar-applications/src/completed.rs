//! Extend the actual Product completion with the native entry's requested scalar observations.
use build_time_evaluation::scalars::CanonicalScalarEvaluationReceipt;
use target_provider_admission::TerminalTargetProviderProposal;

/// The Product phase's completed scalar work and its unchanged provider inputs.
/// Native completion adds application roots and demanded values to this owner.
/// It is never encoded in saved native inputs.
pub struct ProductScalarCompletion {
    pub(super) completed: build_time_evaluation::scalars::ScalarApplicationCompletion,
    pub(super) proposals: Vec<TerminalTargetProviderProposal>,
}

impl ProductScalarCompletion {
    pub fn admitted(&self) -> &psi_scalar_applications::AdmittedScalarApplications {
        self.completed.admitted()
    }
    pub fn proposals(&self) -> &[TerminalTargetProviderProposal] {
        &self.proposals
    }

    /// Add the entry's checked demand while preserving the exact artifact, target and providers.
    pub fn extend(
        self,
        artifact: &terminal_codec::CanonicalTerminalArtifact,
        target: &str,
        proposals: &[TerminalTargetProviderProposal],
        demand: Option<&[semantic_vocabulary::ScalarApplicationId]>,
        accounting: &mut impl Extend<CanonicalScalarEvaluationReceipt>,
    ) -> Result<Option<psi_scalar_applications::AdmittedScalarApplications>, String> {
        if self.completed.artifact().view().identity() != artifact.view().identity()
            || self.completed.primary_target() != Some(target)
        {
            return Err("native scalar completion changed its artifact or target".into());
        }
        if self.proposals.len() != proposals.len()
            || self
                .proposals
                .iter()
                .zip(proposals)
                .any(|(previous, selected)| {
                    let mut expected = previous.clone();
                    expected.application_roots = selected.application_roots.clone();
                    expected != *selected
                })
        {
            return Err("native scalar completion changed its retained provider proposal".into());
        }
        let Some(demand) = demand else {
            return Ok(None);
        };
        let deployed = proposals
            .iter()
            .any(|proposal| proposal.application_roots.is_some());
        let scopes = proposals
            .iter()
            .filter(|proposal| proposal.application_required)
            .map(|proposal| proposal.target.clone())
            .collect::<Vec<_>>();
        let roots = proposals
            .iter()
            .filter_map(|proposal| {
                proposal
                    .application_roots
                    .as_ref()
                    .map(|roots| (proposal.target.clone(), roots.clone()))
            })
            .collect::<Vec<_>>();
        self.completed
            .extend(
                deployed.then_some(scopes.as_slice()),
                &roots,
                demand,
                accounting,
            )
            .map(Some)
    }
}

#[cfg(test)]
mod tests;
