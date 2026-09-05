use terminal_codec::{
    ProofBundleFingerprint, proof_bundle_fingerprint, terminal_psi_identity, validate_debug_map,
};
use terminal_psi::TerminalPsiIdentity;
use terminal_verifier::validate_module_for_optimization;

use super::PsiOptimizationStageError;
use crate::LoweredTerminalPsi;

pub(super) fn validate_carrier(
    lowered: &LoweredTerminalPsi,
) -> Result<(TerminalPsiIdentity, ProofBundleFingerprint), PsiOptimizationStageError> {
    validate_module_for_optimization(&lowered.semantic_module)
        .map_err(PsiOptimizationStageError::InvalidModule)?;
    let semantic = terminal_psi_identity(&lowered.semantic_module)
        .map_err(PsiOptimizationStageError::InvalidSemantic)?;
    let proof = proof_bundle_fingerprint(&lowered.proof_bundle)
        .map_err(PsiOptimizationStageError::InvalidProof)?;
    if let Some(debug_map) = lowered.debug_map.as_ref() {
        validate_debug_map(&lowered.semantic_module, debug_map)
            .map_err(PsiOptimizationStageError::InvalidDebugMap)?;
    }
    Ok((semantic, proof))
}
