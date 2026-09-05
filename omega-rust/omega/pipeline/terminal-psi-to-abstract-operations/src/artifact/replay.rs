use super::error::ArtifactLoweringError;
use super::retention::retain_verified_optimization_input;
use crate::lowering::lower_decoded_verified_module;
use crate::optimization::VerifiedPsiOptimizationInput;
use crate::shared::*;

/// Decode a persisted obligation ledger, reconstruct it from the exact semantic
/// section under the current verifier trust graph, and require exact equality
/// before proof checking or lowering. The producer-authored ledger is never a
/// verdict and cannot choose the proof question.
pub fn lower_replay_artifact_sections(
    semantic_bytes: &[u8],
    obligation_ledger_bytes: &[u8],
    proof_bytes: &[u8],
    profile: &proof_admission::AdmissionProfile,
) -> Result<AbstractOperationPlan, ArtifactLoweringError> {
    let module = terminal_codec::decode_module(semantic_bytes)
        .map_err(ArtifactLoweringError::SemanticDecode)?;
    let obligation_ledger =
        terminal_codec::decode_terminal_obligation_ledger(obligation_ledger_bytes)
            .map_err(ArtifactLoweringError::ObligationLedgerDecode)?;
    let trust_graph = terminal_codec::current_terminal_trust_graph()
        .map_err(ArtifactLoweringError::TrustGraph)?;
    terminal_codec::validate_terminal_obligation_ledger(&obligation_ledger, &module, &trust_graph)
        .map_err(ArtifactLoweringError::ObligationReplay)?;
    let proof = terminal_codec::decode_proof_bundle(proof_bytes)
        .map_err(ArtifactLoweringError::ProofDecode)?;
    if !module.placed_view_inputs.is_empty() {
        return Err(ArtifactLoweringError::PlacedViewInputsRequireCustodyLowering);
    }
    let verified = terminal_verifier::verify_module(&module, &proof, profile)
        .map_err(ArtifactLoweringError::Verification)?;
    lower_decoded_verified_module(&verified, false).map_err(ArtifactLoweringError::Lowering)
}

/// Replay the persisted obligation ledger and retain the complete admitted
/// verifier context required by optimization.
pub fn lower_replay_artifact_sections_for_optimization(
    semantic_bytes: &[u8],
    obligation_ledger_bytes: &[u8],
    proof_bytes: &[u8],
    profile: &proof_admission::AdmissionProfile,
) -> Result<VerifiedPsiOptimizationInput, ArtifactLoweringError> {
    let module = terminal_codec::decode_module(semantic_bytes)
        .map_err(ArtifactLoweringError::SemanticDecode)?;
    let obligation_ledger =
        terminal_codec::decode_terminal_obligation_ledger(obligation_ledger_bytes)
            .map_err(ArtifactLoweringError::ObligationLedgerDecode)?;
    let trust_graph = terminal_codec::current_terminal_trust_graph()
        .map_err(ArtifactLoweringError::TrustGraph)?;
    terminal_codec::validate_terminal_obligation_ledger(&obligation_ledger, &module, &trust_graph)
        .map_err(ArtifactLoweringError::ObligationReplay)?;
    let proof = terminal_codec::decode_proof_bundle(proof_bytes)
        .map_err(ArtifactLoweringError::ProofDecode)?;
    if !module.placed_view_inputs.is_empty() {
        return Err(ArtifactLoweringError::PlacedViewInputsRequireCustodyLowering);
    }
    let verified = terminal_verifier::verify_module_for_optimization(&module, &proof, profile)
        .map_err(ArtifactLoweringError::Verification)?;
    retain_verified_optimization_input(&verified)
}
