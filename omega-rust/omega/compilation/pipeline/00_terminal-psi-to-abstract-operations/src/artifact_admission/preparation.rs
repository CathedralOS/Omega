//! Reusable native admission bound to its exact artifact and proof profile.
//!
//! This retains an unoptimized abstract input. Later optimization selections
//! do not affect admission or lowering and stay with each realization request.

use super::{ArtifactLoweringError, TerminalPlacedViewEstablishment, VerifiedNativeArtifactInput};
use proof_admission::AdmissionProfile;
use terminal_codec::{CanonicalTerminalArtifact, TerminalArtifactIdentity};

/// Target-neutral native input with its exact admission subject and profile.
/// Provider establishments remain bound to this preparation; reopening cannot
/// substitute loans, promote optimizer-only authority, or change the subject.
#[derive(Debug, Clone)]
pub struct PreparedNativeRealizationInput {
    terminal_artifact_identity: TerminalArtifactIdentity,
    profile: AdmissionProfile,
    input: VerifiedNativeArtifactInput,
}

impl PreparedNativeRealizationInput {
    pub const fn terminal_artifact_identity(&self) -> TerminalArtifactIdentity {
        self.terminal_artifact_identity
    }

    pub fn admission_profile(&self) -> &AdmissionProfile {
        &self.profile
    }

    pub fn matches(
        &self,
        terminal_artifact_identity: TerminalArtifactIdentity,
        profile: &AdmissionProfile,
    ) -> bool {
        self.terminal_artifact_identity == terminal_artifact_identity && self.profile == *profile
    }

    /// The admitted provider loans, in the artifact's direct-entry roster order.
    pub fn placed_view_establishments(&self) -> &[TerminalPlacedViewEstablishment] {
        self.input.placed_view_establishments()
    }

    /// Reopen only the exact admitted artifact/profile, preserving bound loans.
    pub fn reopen(
        &self,
        artifact: &CanonicalTerminalArtifact,
        profile: &AdmissionProfile,
    ) -> Result<VerifiedNativeArtifactInput, &'static str> {
        if !self.matches(artifact.manifest().identity(), profile) {
            return Err("Terminal artifact identity or proof-admission profile changed");
        }
        Ok(self.input.clone())
    }
}

/// Admit and retain one canonical artifact for repeated native realization.
/// Missing, duplicate, foreign or invalid placed-view supplies reject through
/// the same admission as an uncached native input. No optimization runs here.
pub fn prepare_native_realization_input(
    artifact: &CanonicalTerminalArtifact,
    profile: &AdmissionProfile,
    placed_view_establishments: &[TerminalPlacedViewEstablishment],
) -> Result<PreparedNativeRealizationInput, ArtifactLoweringError> {
    let input =
        VerifiedNativeArtifactInput::prepare(artifact, profile, placed_view_establishments)?;
    Ok(PreparedNativeRealizationInput {
        terminal_artifact_identity: artifact.manifest().identity(),
        profile: profile.clone(),
        input,
    })
}
