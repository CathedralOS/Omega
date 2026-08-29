//! Artifact entrance: canonical decode and verification before either ordinary
//! lowering or explicit optimizer-context retention.

mod error;
mod native;
mod ranked_native;
mod replay;
mod retention;

pub use error::ArtifactLoweringError;
pub use native::{NativeArtifactOperationPlan, lower_artifact_sections_for_native_realization};
pub use ranked_native::lower_artifact_sections_for_native_ranked_countdown;
pub use replay::{lower_replay_artifact_sections, lower_replay_artifact_sections_for_optimization};

use crate::optimization::VerifiedPsiOptimizationInput;
use crate::shared::*;
use retention::retain_verified_optimization_input;

/// Canonical-decode and verify terminal-Psi semantic/proof artifact sections
/// before constructing Omega's source-independent realization requirements.
/// Producer-owned modules and frontend trees cannot cross this boundary.
pub fn lower_artifact_sections(
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    profile: &psi_proof_admission::AdmissionProfile,
) -> Result<AbstractOperationPlan, ArtifactLoweringError> {
    let module = psi_terminal_codec::decode_module(semantic_bytes)
        .map_err(ArtifactLoweringError::SemanticDecode)?;
    let proof = psi_terminal_codec::decode_proof_bundle(proof_bytes)
        .map_err(ArtifactLoweringError::ProofDecode)?;
    native::lower_decoded_ordinary_module(&module, &proof, profile)
}

/// Construct the required optimizer carrier without affecting the ordinary
/// empty-selection path. This API intentionally repeats canonical artifact
/// admission only when an optimizer consumer explicitly asks for it.
pub fn lower_artifact_sections_for_optimization(
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    profile: &psi_proof_admission::AdmissionProfile,
) -> Result<VerifiedPsiOptimizationInput, ArtifactLoweringError> {
    let module = psi_terminal_codec::decode_module(semantic_bytes)
        .map_err(ArtifactLoweringError::SemanticDecode)?;
    let proof = psi_terminal_codec::decode_proof_bundle(proof_bytes)
        .map_err(ArtifactLoweringError::ProofDecode)?;
    let verified = psi_terminal_verifier::verify_module(&module, &proof, profile)
        .map_err(ArtifactLoweringError::Verification)?;
    retain_verified_optimization_input(&verified)
}
