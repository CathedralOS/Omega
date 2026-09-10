//! Native artifact authority routing after canonical decode.

use abstract_operations::{AbstractOperationPlan, RankedNativeAbstractOperationPlan};

use crate::lowering::lower_decoded_verified_module;

use super::{ArtifactLoweringError, ranked_native};

/// Ordinary verified graphs (including natural-ranked cycles) and the legacy
/// countdown's specialized native custody. Dispatch is decided from the decoded
/// Terminal module; callers cannot reinterpret one carrier as the other.
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

/// Decode one canonical artifact and select its only valid unoptimized native
/// authority path. Legacy countdown input never falls back to ordinary
/// admission. Natural-ranked input uses ordinary grouped proof verification,
/// without being probed against the countdown's fixed-work exception.
pub fn lower_artifact_sections_for_native_realization(
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    profile: &proof_admission::AdmissionProfile,
) -> Result<NativeArtifactOperationPlan, ArtifactLoweringError> {
    let module = terminal_codec::decode_module(semantic_bytes)
        .map_err(ArtifactLoweringError::SemanticDecode)?;
    let proof = terminal_codec::decode_proof_bundle(proof_bytes)
        .map_err(ArtifactLoweringError::ProofDecode)?;
    if !module.placed_view_inputs.is_empty() {
        return Err(ArtifactLoweringError::PlacedViewInputsRequireCustodyLowering);
    }
    if module.machines.iter().any(|machine| {
        machine
            .ranked_scc
            .as_ref()
            .is_some_and(|ranking| ranking.as_unsigned_countdown().is_some())
    }) {
        ranked_native::lower_decoded_native_ranked_countdown(&module, &proof, profile)
            .map(NativeArtifactOperationPlan::RankedU32Countdown)
    } else {
        lower_decoded_ordinary_module(&module, &proof, profile)
            .map(NativeArtifactOperationPlan::Ordinary)
    }
}

pub(super) fn lower_decoded_ordinary_module(
    module: &terminal_psi::TerminalModule,
    proof: &terminal_verifier::ProofBundle,
    profile: &proof_admission::AdmissionProfile,
) -> Result<AbstractOperationPlan, ArtifactLoweringError> {
    let verified = terminal_verifier::verify_module(module, proof, profile)
        .map_err(ArtifactLoweringError::Verification)?;
    lower_decoded_verified_module(&verified).map_err(ArtifactLoweringError::Lowering)
}
