//! Explicit scalar demand between native preparation and native lowering.
//!
//! The application sponsors evaluation. Native compilation retains only the
//! request and completed judgment, never an evaluator or a callback into Psi.

use diagnostics::Diagnostic;
use std::sync::Arc;
use terminal_psi_to_abstract_operations::{
    NativeScalarApplications, TerminalNativeRealizationProposal,
};

pub struct NativeScalarCompletion<'input> {
    artifact: &'input Arc<terminal_codec::CanonicalTerminalArtifact>,
    proposal: &'input TerminalNativeRealizationProposal,
    demand: Vec<semantic_vocabulary::ScalarApplicationId>,
    completed: Option<terminal_installation::AdmittedScalarApplications>,
}

impl<'input> NativeScalarCompletion<'input> {
    pub(crate) fn new(
        artifact: &'input Arc<terminal_codec::CanonicalTerminalArtifact>,
        proposal: &'input TerminalNativeRealizationProposal,
        applications: impl IntoIterator<Item = semantic_vocabulary::ScalarApplicationId>,
    ) -> Self {
        let mut demand = Vec::new();
        for application in applications {
            if !demand.contains(&application) {
                demand.push(application);
            }
        }
        Self {
            artifact,
            proposal,
            demand,
            completed: None,
        }
    }

    pub fn artifact(&self) -> &Arc<terminal_codec::CanonicalTerminalArtifact> {
        self.artifact
    }

    pub fn target(&self) -> &str {
        self.proposal.target_profile().target_name()
    }

    pub fn demand(&self) -> &[semantic_vocabulary::ScalarApplicationId] {
        &self.demand
    }

    pub(crate) fn is_required(&self) -> bool {
        !self.demand.is_empty()
            || !self
                .artifact
                .semantic_module()
                .scalar_applications
                .conditions
                .is_empty()
    }

    pub fn provider_proposals(&self) -> &[backend_plan::TerminalTargetProviderProposal] {
        self.proposal.target_provider_proposals()
    }

    /// Retain the application-produced judgment. Native lowering independently
    /// binds it to the artifact, original provider companion and exact demand.
    pub fn complete(
        &mut self,
        completed: terminal_installation::AdmittedScalarApplications,
    ) -> Result<(), String> {
        if self.completed.is_some() {
            return Err("native scalar work was completed more than once".into());
        }
        self.completed = Some(completed);
        Ok(())
    }

    pub(crate) fn native_input(
        &self,
    ) -> Result<Option<NativeScalarApplications<'_>>, Vec<Diagnostic>> {
        if !self.is_required() {
            return Ok(None);
        }
        let completed = self
            .completed
            .as_ref()
            .ok_or_else(|| vec![Diagnostic::error("native scalar work has not completed")])?;
        let selection = self
            .provider_proposals()
            .iter()
            .find(|proposal| proposal.target == self.target())
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "native scalar work lost its target provider companion",
                )]
            })?
            .selection
            .as_ref()
            .map_err(|reasons| {
                reasons
                    .iter()
                    .cloned()
                    .map(Diagnostic::error)
                    .collect::<Vec<_>>()
            })?;
        Ok(Some(NativeScalarApplications {
            completed,
            provider_declarations: &selection.plans,
        }))
    }
}
