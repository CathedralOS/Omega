use super::error::ArtifactLoweringError;
use crate::lowering::lower_decoded_optimizable_module;
use crate::optimization::{VerifiedPsiOptimizationContext, VerifiedPsiOptimizationInput};
use crate::shared::*;

pub(super) fn retain_verified_optimization_input(
    verified: &VerifiedOptimizableTerminalModule<'_>,
) -> Result<VerifiedPsiOptimizationInput, ArtifactLoweringError> {
    let plan =
        lower_decoded_optimizable_module(verified).map_err(ArtifactLoweringError::Lowering)?;
    let proof_bundle_fingerprint =
        terminal_codec::proof_bundle_fingerprint(verified.proof_bundle())
            .map_err(ArtifactLoweringError::ProofFingerprint)?;
    Ok(VerifiedPsiOptimizationInput {
        plan,
        context: VerifiedPsiOptimizationContext {
            module: verified.module().clone(),
            proof_bundle: verified.proof_bundle().clone(),
            proof_bundle_fingerprint,
            reconstructed_obligations: verified.reconstructed_obligations().clone(),
            accepted_facts: verified.accepted_facts().to_vec(),
            structural_frontiers: verified.structural_frontiers().clone(),
        },
    })
}
