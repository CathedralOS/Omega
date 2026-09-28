//! Prepare the exact Psi artifact and companion bytes before any output is staged.
//!
//! Construction replays the standalone receiver under the offered claim's policy.
//! This establishes producer consistency, not admission by a deployment policy.

use proof_admission::AdmissionProfile;

use crate::{
    CanonicalTerminalArtifact, CodecError, PccReceiverPolicy, PccVerificationOutcome,
    build_psi_proof_sidecar, verify_psi_proof_sidecar,
};

/// An immutable artifact/companion pair whose bounded Psi claim passed replay.
/// Filesystem destinations, staging and installation belong to the caller.
#[derive(Debug)]
pub struct PreparedPsiProofPair {
    artifact_bytes: Vec<u8>,
    sidecar_bytes: Vec<u8>,
}

#[derive(Debug)]
pub enum PsiProofPairPreparationError {
    SidecarConstruction(CodecError),
    ProducerValidation(PccVerificationOutcome),
}

impl PreparedPsiProofPair {
    /// Serialize, construct evidence for, and independently replay the exact pair.
    /// No bytes are returned as prepared when receiver checking rejects or is incomplete.
    pub fn new(
        artifact: &CanonicalTerminalArtifact,
        admission_profile: &AdmissionProfile,
    ) -> Result<Self, PsiProofPairPreparationError> {
        let artifact_bytes = artifact.to_bytes();
        let sidecar = build_psi_proof_sidecar(artifact, admission_profile, &artifact_bytes)
            .map_err(PsiProofPairPreparationError::SidecarConstruction)?;
        let sidecar_bytes = sidecar.to_bytes();
        let policy = PccReceiverPolicy::for_offered_claim(&sidecar, admission_profile.clone());
        match verify_psi_proof_sidecar(&artifact_bytes, &sidecar_bytes, &policy) {
            PccVerificationOutcome::Complete(_) => Ok(Self {
                artifact_bytes,
                sidecar_bytes,
            }),
            outcome => Err(PsiProofPairPreparationError::ProducerValidation(outcome)),
        }
    }

    pub fn artifact_bytes(&self) -> &[u8] {
        &self.artifact_bytes
    }

    pub fn sidecar_bytes(&self) -> &[u8] {
        &self.sidecar_bytes
    }
}

impl std::fmt::Display for PsiProofPairPreparationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SidecarConstruction(error) => {
                write!(formatter, "cannot build the psi proof sidecar: {error}")
            }
            Self::ProducerValidation(outcome) => {
                write!(
                    formatter,
                    "psi proof sidecar failed producer validation: {outcome:?}"
                )
            }
        }
    }
}

impl std::error::Error for PsiProofPairPreparationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SidecarConstruction(error) => Some(error),
            Self::ProducerValidation(_) => None,
        }
    }
}
