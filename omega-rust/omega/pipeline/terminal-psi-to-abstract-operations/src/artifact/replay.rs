use super::error::ArtifactLoweringError;
use super::retention::{
    VerifiedPsiOptimizationInputWithPlacedViewInputs, retain_verified_optimization_input,
};
use crate::lowering::lower_decoded_verified_module;
use crate::optimization::VerifiedPsiOptimizationInput;
use crate::shared::*;
use abstract_operations::AbstractOperationPlanWithPlacedViewInputs;

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
    lower_decoded_verified_module(&verified).map_err(ArtifactLoweringError::Lowering)
}

/// Decode a persisted obligation ledger, replay it against the exact semantic
/// section, verify, lower, and retain the plan-laid input roster beside the
/// ordinary abstract plan. The ledger's canonical program fingerprint already
/// binds every retained row, so a stale or substituted roster cannot replay
/// against these bytes. The rows are semantic custody only: this stage
/// neither supplies backing nor emits an access event.
pub fn lower_replay_artifact_sections_with_placed_view_inputs(
    semantic_bytes: &[u8],
    obligation_ledger_bytes: &[u8],
    proof_bytes: &[u8],
    profile: &proof_admission::AdmissionProfile,
) -> Result<AbstractOperationPlanWithPlacedViewInputs, ArtifactLoweringError> {
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
    let verified = terminal_verifier::verify_module(&module, &proof, profile)
        .map_err(ArtifactLoweringError::Verification)?;
    let plan = lower_decoded_verified_module(&verified).map_err(ArtifactLoweringError::Lowering)?;
    Ok(AbstractOperationPlanWithPlacedViewInputs {
        plan,
        placed_view_inputs: module.placed_view_inputs,
    })
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

/// Decode a persisted obligation ledger, replay it against the exact semantic
/// section, verify for optimizer admission, and retain the plan-laid input
/// roster beside the optimizer input. The ledger's canonical program
/// fingerprint already binds every retained row, so a stale or substituted
/// roster cannot replay against these bytes. The rows are semantic custody
/// only: this stage neither supplies backing nor emits an access event.
pub fn lower_replay_artifact_sections_for_optimization_with_placed_view_inputs(
    semantic_bytes: &[u8],
    obligation_ledger_bytes: &[u8],
    proof_bytes: &[u8],
    profile: &proof_admission::AdmissionProfile,
) -> Result<VerifiedPsiOptimizationInputWithPlacedViewInputs, ArtifactLoweringError> {
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
    let verified = terminal_verifier::verify_module_for_optimization(&module, &proof, profile)
        .map_err(ArtifactLoweringError::Verification)?;
    let input = retain_verified_optimization_input(&verified)?;
    Ok(VerifiedPsiOptimizationInputWithPlacedViewInputs {
        input,
        placed_view_inputs: module.placed_view_inputs,
    })
}
