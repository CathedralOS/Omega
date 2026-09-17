//! Native authority is established by ordinary verification, never by promotion
//! of optimizer-only input.

use super::retention::{AdmittedOptimizationArtifact, retain_verified_optimization_context};
use super::{ArtifactLoweringError, require_empty_placed_view_inputs};
use crate::lowering::lower_decoded_verified_module;
use crate::optimization::{VerifiedPsiOptimizationContext, VerifiedPsiOptimizationInput};
use abstract_operations::AbstractOperationPlan;

/// Native-admitted input for consumers without placed-view custody support.
/// Construction is private and requires an empty placed-view roster.
#[derive(Debug, Clone)]
pub struct VerifiedNativeArtifactInput {
    optimization_input: VerifiedPsiOptimizationInput,
}

impl VerifiedNativeArtifactInput {
    pub fn plan(&self) -> &AbstractOperationPlan {
        self.optimization_input.plan()
    }

    pub const fn context(&self) -> &VerifiedPsiOptimizationContext {
        self.optimization_input.context()
    }

    pub fn into_optimization_input(self) -> VerifiedPsiOptimizationInput {
        self.optimization_input
    }
}

/// One canonical native-admitted plan with its exact retained verifier context.
/// Placed-view rows are semantic custody, not backing or access-event authority.
#[derive(Debug, Clone)]
pub struct AdmittedNativeArtifact {
    optimization_input: VerifiedPsiOptimizationInput,
}

impl AdmittedNativeArtifact {
    pub fn plan(&self) -> &AbstractOperationPlan {
        self.optimization_input.plan()
    }

    pub const fn context(&self) -> &VerifiedPsiOptimizationContext {
        self.optimization_input.context()
    }

    pub fn placed_view_inputs(&self) -> &[terminal_psi::TerminalPlacedViewInput] {
        &self.context().module().placed_view_inputs
    }

    /// Downgrade authority while preserving the complete semantic roster.
    pub fn into_optimization_artifact(self) -> AdmittedOptimizationArtifact {
        AdmittedOptimizationArtifact {
            input: self.optimization_input,
        }
    }

    /// The executable-image realization input. It stays fail-closed for a
    /// nonempty roster because an image entry has no pointer source: the
    /// placed-entry ABI derived from the exact placement plan needs a caller
    /// that lends the referent address, and an emitted `_start` shim has
    /// none until a provider establishment route supplies it. Custody-aware
    /// consumers take `into_optimization_artifact` instead; native execution
    /// of the derived ABI with a lent host referent is exercised by the
    /// compiler control `direct_placed_view_input_survives_codec_and_native_replay`.
    pub fn try_into_native_input(
        self,
    ) -> Result<VerifiedNativeArtifactInput, ArtifactLoweringError> {
        require_empty_placed_view_inputs(self.placed_view_inputs())?;
        Ok(VerifiedNativeArtifactInput {
            optimization_input: self.optimization_input,
        })
    }
}

pub(super) fn lower_decoded_native_module(
    module: &terminal_psi::TerminalModule,
    proof: &terminal_verifier::ProofBundle,
    profile: &proof_admission::AdmissionProfile,
) -> Result<AdmittedNativeArtifact, ArtifactLoweringError> {
    let verified = terminal_verifier::verify_module(module, proof, profile)
        .map_err(ArtifactLoweringError::Verification)?;
    let plan = lower_decoded_verified_module(&verified).map_err(ArtifactLoweringError::Lowering)?;
    let optimizable = verified
        .into_optimization()
        .map_err(ArtifactLoweringError::Verification)?;
    let context = retain_verified_optimization_context(&optimizable)?;
    Ok(AdmittedNativeArtifact {
        optimization_input: VerifiedPsiOptimizationInput { plan, context },
    })
}
