//! Artifact entrance: canonical decode and verification before either ordinary
//! lowering or explicit optimizer-context retention.

mod error;
mod ranked_native;
mod replay;
mod retention;

pub use error::ArtifactLoweringError;
pub use ranked_native::lower_artifact_sections_for_native_ranked_countdown;
pub use replay::{lower_replay_artifact_sections, lower_replay_artifact_sections_for_optimization};

use crate::lowering::lower_decoded_verified_module;
use crate::optimization::VerifiedPsiOptimizationInput;
use crate::shared::*;
use omega_abstract_operations::RankedNativeAbstractOperationPlan;
use retention::retain_verified_optimization_input;

/// The two disjoint authority carriers accepted by the unoptimized native
/// realization entrance. Dispatch is decided from the decoded Terminal module;
/// callers cannot reinterpret ordinary authority as ranked authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeArtifactOperationPlan {
    Ordinary(AbstractOperationPlan),
    RankedU32Countdown(RankedNativeAbstractOperationPlan),
}

impl NativeArtifactOperationPlan {
    pub const fn plan(&self) -> &AbstractOperationPlan {
        match self {
            Self::Ordinary(plan) => plan,
            Self::RankedU32Countdown(ranked) => &ranked.plan,
        }
    }
}

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
    lower_decoded_ordinary_module(&module, &proof, profile)
}

/// Decode one canonical artifact and select its only valid unoptimized native
/// authority path. Ranked input never falls back to ordinary admission, and an
/// ordinary module is never probed against the ranked exception.
pub fn lower_artifact_sections_for_native_realization(
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    profile: &psi_proof_admission::AdmissionProfile,
) -> Result<NativeArtifactOperationPlan, ArtifactLoweringError> {
    let module = psi_terminal_codec::decode_module(semantic_bytes)
        .map_err(ArtifactLoweringError::SemanticDecode)?;
    let proof = psi_terminal_codec::decode_proof_bundle(proof_bytes)
        .map_err(ArtifactLoweringError::ProofDecode)?;
    if module
        .machines
        .iter()
        .any(|machine| machine.ranked_scc.is_some())
    {
        ranked_native::lower_decoded_native_ranked_countdown(&module, &proof, profile)
            .map(NativeArtifactOperationPlan::RankedU32Countdown)
    } else {
        lower_decoded_ordinary_module(&module, &proof, profile)
            .map(NativeArtifactOperationPlan::Ordinary)
    }
}

fn lower_decoded_ordinary_module(
    module: &psi_terminal::TerminalModule,
    proof: &psi_terminal_verifier::ProofBundle,
    profile: &psi_proof_admission::AdmissionProfile,
) -> Result<AbstractOperationPlan, ArtifactLoweringError> {
    let verified = psi_terminal_verifier::verify_module(module, proof, profile)
        .map_err(ArtifactLoweringError::Verification)?;
    lower_decoded_verified_module(&verified, false).map_err(ArtifactLoweringError::Lowering)
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
