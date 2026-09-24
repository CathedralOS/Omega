use super::error::ArtifactLoweringError;
use crate::optimization::{VerifiedPsiOptimizationContext, VerifiedPsiOptimizationInput};
use abstract_operations::AbstractOperationPlan;
use terminal_verifier::VerifiedOptimizableTerminalModule;

/// One canonical optimizer-admitted program plus its exact plan-laid input
/// roster. Construction is private to artifact admission — it exists only as
/// the downgrade of a native admission — so the roster cannot be paired with
/// an unrelated verified input. The rows stay beside the verified optimizer
/// input as semantic custody: they grant no backing, lifetime, or
/// access-event authority, and remain visible inside the retained verifier
/// context module.
///
/// Optimizer authority has no conversion to native authority:
/// ```compile_fail
/// use terminal_psi_to_abstract_operations::{AdmittedNativeArtifact, AdmittedOptimizationArtifact};
/// fn promote(input: AdmittedOptimizationArtifact) -> AdmittedNativeArtifact {
///     input
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedOptimizationArtifact {
    pub(super) input: VerifiedPsiOptimizationInput,
}

impl AdmittedOptimizationArtifact {
    pub const fn plan(&self) -> &AbstractOperationPlan {
        self.input.plan()
    }

    pub const fn context(&self) -> &VerifiedPsiOptimizationContext {
        self.input.context()
    }

    pub fn placed_view_inputs(&self) -> &[terminal_psi::TerminalPlacedViewInput] {
        &self.context().module().placed_view_inputs
    }

    /// The verified optimizer input with the complete roster still retained
    /// inside the verifier context module. The handoff stays exact because
    /// construction is private to artifact admission, so a stale or
    /// substituted roster is unrepresentable here rather than merely
    /// unchecked. The rows remain semantic custody only: they grant no
    /// backing, range, access, or lifetime authority.
    pub fn into_optimization_input(self) -> VerifiedPsiOptimizationInput {
        self.input
    }
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
