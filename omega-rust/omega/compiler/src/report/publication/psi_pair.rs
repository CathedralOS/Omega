//! Prepare and stage the exact Psi artifact/proof pair requested for publication.
//!
//! Terminal-only and native publication use the same construction and receiver
//! replay before staging either member. Flat products stage both members before
//! installation begins; macOS publication places these same bytes in its staged
//! package tree. Receipt paths and sizes derive from this retained pair.

use std::path::Path;

use super::{StagedProduct, appended_file_name_path, stage_exact_bytes};
use crate::report::PccPublicationReceipt;

pub(crate) struct PreparedPsiProofPair {
    artifact_bytes: Vec<u8>,
    sidecar_bytes: Vec<u8>,
}

impl PreparedPsiProofPair {
    pub(crate) fn new(
        artifact: &terminal_codec::CanonicalTerminalArtifact,
        admission_profile: &proof_admission::AdmissionProfile,
    ) -> Result<Self, String> {
        let artifact_bytes = artifact.to_bytes();
        let sidecar =
            terminal_codec::build_psi_proof_sidecar(artifact, admission_profile, &artifact_bytes)
                .map_err(|error| format!("cannot build the psi proof sidecar: {error}"))?;
        let sidecar_bytes = sidecar.to_bytes();
        validate_psi_pair(&artifact_bytes, &sidecar, admission_profile)?;
        Ok(Self {
            artifact_bytes,
            sidecar_bytes,
        })
    }

    pub(crate) fn artifact_bytes(&self) -> &[u8] {
        &self.artifact_bytes
    }

    pub(crate) fn sidecar_bytes(&self) -> &[u8] {
        &self.sidecar_bytes
    }

    /// Describe the selected destination; callers retain this receipt only after
    /// the staged file set or package tree has installed successfully.
    pub(crate) fn receipt(&self, artifact_path: &Path) -> PccPublicationReceipt {
        PccPublicationReceipt {
            product: terminal_codec::PccProductKind::Psi,
            artifact_path: artifact_path.to_path_buf(),
            artifact_byte_len: self.artifact_bytes.len() as u64,
            sidecar_path: appended_file_name_path(artifact_path, ".proof"),
            sidecar_byte_len: self.sidecar_bytes.len() as u64,
        }
    }

    /// Append both staged members without installing either destination. If the
    /// second staging fails, the previously published artifact/proof pair remains
    /// untouched; the caller receives no successful pair receipt.
    pub(crate) fn stage(
        &self,
        artifact_path: &Path,
        staged_products: &mut Vec<StagedProduct>,
    ) -> Result<PccPublicationReceipt, String> {
        let receipt = self.receipt(artifact_path);
        staged_products.push(stage_exact_bytes(
            artifact_path,
            &self.artifact_bytes,
            false,
        )?);
        staged_products.push(stage_exact_bytes(
            &receipt.sidecar_path,
            &self.sidecar_bytes,
            false,
        )?);
        Ok(receipt)
    }
}

/// Producer-side pair validation for one Psi artifact/companion: replay the
/// exact checks a receiver performs under a self-consistent policy so a
/// malformed or internally inconsistent pair can never be reported as
/// published.
fn validate_psi_pair(
    psi_bytes: &[u8],
    sidecar: &terminal_codec::PccProofSidecar,
    admission_profile: &proof_admission::AdmissionProfile,
) -> Result<(), String> {
    let policy =
        terminal_codec::PccReceiverPolicy::for_offered_claim(sidecar, admission_profile.clone());
    match terminal_codec::verify_psi_proof_sidecar(psi_bytes, &sidecar.to_bytes(), &policy) {
        terminal_codec::PccVerificationOutcome::Complete(_) => Ok(()),
        outcome => Err(format!(
            "psi proof sidecar failed producer validation: {outcome:?}"
        )),
    }
}
