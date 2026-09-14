use super::error::ArtifactLoweringError;
use crate::lowering::lower_decoded_optimizable_module;
use crate::optimization::{VerifiedPsiOptimizationContext, VerifiedPsiOptimizationInput};
use crate::shared::*;

/// One canonical optimizer-admitted program plus its exact plan-laid input
/// roster. Construction is private to artifact admission so the roster cannot
/// be paired with an unrelated verified input. The rows stay beside the
/// verified optimizer input as semantic custody: they grant no backing,
/// lifetime, or access-event authority, and remain visible inside the
/// retained verifier context module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedPsiOptimizationInputWithPlacedViewInputs {
    pub(crate) input: VerifiedPsiOptimizationInput,
    pub(crate) placed_view_inputs: Vec<terminal_psi::TerminalPlacedViewInput>,
}

impl VerifiedPsiOptimizationInputWithPlacedViewInputs {
    pub const fn plan(&self) -> &AbstractOperationPlan {
        self.input.plan()
    }

    pub const fn context(&self) -> &VerifiedPsiOptimizationContext {
        self.input.context()
    }

    pub fn placed_view_inputs(&self) -> &[terminal_psi::TerminalPlacedViewInput] {
        &self.placed_view_inputs
    }

    /// Release roster custody explicitly. The returned input cannot rejoin a
    /// plan-laid input downstream; callers needing the roster must retain
    /// this carrier instead.
    pub fn into_optimization_input(self) -> VerifiedPsiOptimizationInput {
        self.input
    }
}

pub(super) fn retain_verified_optimization_input(
    verified: &VerifiedOptimizableTerminalModule<'_>,
) -> Result<VerifiedPsiOptimizationInput, ArtifactLoweringError> {
    let plan =
        lower_decoded_optimizable_module(verified).map_err(ArtifactLoweringError::Lowering)?;
    let context = retain_verified_optimization_context(verified)?;
    Ok(VerifiedPsiOptimizationInput { plan, context })
}

pub(super) fn retain_verified_optimization_context(
    verified: &VerifiedOptimizableTerminalModule<'_>,
) -> Result<VerifiedPsiOptimizationContext, ArtifactLoweringError> {
    let proof_bundle_fingerprint =
        terminal_codec::proof_bundle_fingerprint(verified.proof_bundle())
            .map_err(ArtifactLoweringError::ProofFingerprint)?;
    Ok(VerifiedPsiOptimizationContext {
        module: verified.module().clone(),
        proof_bundle: verified.proof_bundle().clone(),
        proof_bundle_fingerprint,
        reconstructed_obligations: verified.reconstructed_obligations().clone(),
        accepted_facts: verified.accepted_facts().to_vec(),
        structural_frontiers: verified.structural_frontiers().clone(),
    })
}
