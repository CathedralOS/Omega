//! Native artifact authority routing after canonical decode.

use abstract_operations::AbstractOperationPlan;

use super::retention::retain_verified_optimization_context;
use crate::lowering::lower_decoded_verified_module;
use crate::optimization::{VerifiedPsiOptimizationContext, VerifiedPsiOptimizationInput};

use super::ArtifactLoweringError;

/// One canonical native-admitted program with its checked optimizer context.
/// Construction is private to native artifact admission; a caller cannot pair
/// an unrelated plan with evidence or promote optimizer-only authority.
#[derive(Debug, Clone)]
pub struct VerifiedNativeArtifactInput {
    optimization_input: VerifiedPsiOptimizationInput,
}

impl VerifiedNativeArtifactInput {
    pub fn plan(&self) -> &AbstractOperationPlan {
        self.optimization_input.plan()
    }

    /// The verifier-retained context this exact plan was lowered from. The
    /// module inside remains immutable evidence, not an admission authority.
    pub const fn context(&self) -> &VerifiedPsiOptimizationContext {
        self.optimization_input.context()
    }

    pub fn into_optimization_input(self) -> VerifiedPsiOptimizationInput {
        self.optimization_input
    }
}

/// One canonical native-admitted program plus its exact plan-laid input
/// roster. The roster stays beside the verified native authority as semantic
/// custody: it grants no backing, lifetime, or access-event authority, and
/// remains visible inside the retained verifier context module.
#[derive(Debug, Clone)]
pub struct VerifiedNativeArtifactInputWithPlacedViewInputs {
    input: VerifiedNativeArtifactInput,
    placed_view_inputs: Vec<terminal_psi::TerminalPlacedViewInput>,
}

impl VerifiedNativeArtifactInputWithPlacedViewInputs {
    pub fn plan(&self) -> &AbstractOperationPlan {
        self.input.plan()
    }

    pub fn placed_view_inputs(&self) -> &[terminal_psi::TerminalPlacedViewInput] {
        &self.placed_view_inputs
    }

    pub fn into_optimization_input(self) -> VerifiedPsiOptimizationInput {
        self.input.into_optimization_input()
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
) -> Result<VerifiedNativeArtifactInput, ArtifactLoweringError> {
    let module = terminal_codec::decode_module(semantic_bytes)
        .map_err(ArtifactLoweringError::SemanticDecode)?;
    let proof = terminal_codec::decode_proof_bundle(proof_bytes)
        .map_err(ArtifactLoweringError::ProofDecode)?;
    if !module.placed_view_inputs.is_empty() {
        return Err(ArtifactLoweringError::PlacedViewInputsRequireCustodyLowering);
    }
    lower_decoded_native_module(&module, &proof, profile)
}

/// Decode one canonical artifact, admit it for native realization, and retain
/// its exact plan-laid input roster beside the verified native authority. The
/// roster remains semantic custody only: this stage neither supplies backing
/// nor emits an access event.
pub fn lower_artifact_sections_for_native_realization_with_placed_view_inputs(
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    profile: &proof_admission::AdmissionProfile,
) -> Result<VerifiedNativeArtifactInputWithPlacedViewInputs, ArtifactLoweringError> {
    let module = terminal_codec::decode_module(semantic_bytes)
        .map_err(ArtifactLoweringError::SemanticDecode)?;
    let proof = terminal_codec::decode_proof_bundle(proof_bytes)
        .map_err(ArtifactLoweringError::ProofDecode)?;
    let input = lower_decoded_native_module(&module, &proof, profile)?;
    Ok(VerifiedNativeArtifactInputWithPlacedViewInputs {
        input,
        placed_view_inputs: module.placed_view_inputs,
    })
}

fn lower_decoded_native_module(
    module: &terminal_psi::TerminalModule,
    proof: &terminal_verifier::ProofBundle,
    profile: &proof_admission::AdmissionProfile,
) -> Result<VerifiedNativeArtifactInput, ArtifactLoweringError> {
    if module.machines.iter().any(|machine| {
        machine
            .ranked_scc
            .as_ref()
            .is_some_and(|ranking| ranking.as_unsigned_countdown().is_some())
    }) {
        Err(ArtifactLoweringError::UnsupportedUnsignedCountdownNativeCustody)
    } else {
        let verified = terminal_verifier::verify_module(module, proof, profile)
            .map_err(ArtifactLoweringError::Verification)?;
        let plan =
            lower_decoded_verified_module(&verified).map_err(ArtifactLoweringError::Lowering)?;
        let optimizable = verified
            .into_optimization()
            .map_err(ArtifactLoweringError::Verification)?;
        let context = retain_verified_optimization_context(&optimizable)?;
        Ok(VerifiedNativeArtifactInput {
            optimization_input: VerifiedPsiOptimizationInput { plan, context },
        })
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
