//! Bind producer-retained application joins to the published artifact.
//!
//! Canonical requirement and argument rows already belong to Terminal. This
//! temporary source companion is needed only while selected realization still
//! consumes checked declarations; publication does not reconstruct the demands.

mod result_policies;

use checked_trees::CheckedTrees;
use lowered_psi::{LoweredBoundaryApplicationOccurrence, LoweredSourceCustody};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedBoundaryOperatorApplicationScope {
    terminal_artifact_identity: terminal_codec::TerminalArtifactIdentity,
    applications: Vec<checked_trees::CheckedBoundaryOperatorApplicationDemand>,
    occurrences: Vec<LoweredBoundaryApplicationOccurrence>,
}

impl CheckedBoundaryOperatorApplicationScope {
    pub fn validate_for_artifact(
        &self,
        artifact: &terminal_codec::CanonicalTerminalArtifact,
    ) -> Result<(), &'static str> {
        if self.terminal_artifact_identity != artifact.manifest().identity() {
            return Err("checked boundary-operator scope belongs to a different Terminal artifact");
        }
        Ok(())
    }

    pub fn applications(&self) -> &[checked_trees::CheckedBoundaryOperatorApplicationDemand] {
        &self.applications
    }

    pub fn is_empty(&self) -> bool {
        self.occurrences.is_empty()
    }

    pub fn occurrences(&self) -> &[LoweredBoundaryApplicationOccurrence] {
        &self.occurrences
    }
}

pub fn checked_boundary_operator_scope(
    checked: &CheckedTrees,
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    source_custody: &LoweredSourceCustody,
) -> Result<CheckedBoundaryOperatorApplicationScope, &'static str> {
    result_policies::replay(checked, artifact.semantic_module(), source_custody)?;
    Ok(CheckedBoundaryOperatorApplicationScope {
        terminal_artifact_identity: artifact.manifest().identity(),
        applications: checked.facts.operators.boundary_applications.clone(),
        occurrences: source_custody.boundary_application_occurrences.clone(),
    })
}
