//! Native artifact authority routing after canonical decode.

use abstract_operations::AbstractOperationPlan;

use crate::lowering::lower_decoded_verified_module;

use super::ArtifactLoweringError;

/// Decode one canonical artifact and select its only valid unoptimized native
/// authority path. Legacy countdown input never falls back to ordinary
/// admission. Natural-ranked input uses ordinary grouped proof verification,
/// without being probed against the countdown's fixed-work exception.
pub fn lower_artifact_sections_for_native_realization(
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    profile: &proof_admission::AdmissionProfile,
) -> Result<AbstractOperationPlan, ArtifactLoweringError> {
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
        Err(ArtifactLoweringError::UnsupportedUnsignedCountdownNativeCustody)
    } else {
        lower_decoded_ordinary_module(&module, &proof, profile)
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
