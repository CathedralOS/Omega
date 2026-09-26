//! Native authority is established by ordinary verification, never by promotion
//! of optimizer-only input.

use super::ArtifactLoweringError;
use super::retention::{AdmittedOptimizationArtifact, retain_verified_optimization_context};
use crate::lowering::lower_decoded_verified_module;
use crate::optimization::{VerifiedPsiOptimizationContext, VerifiedPsiOptimizationInput};
use abstract_operations::AbstractOperationPlan;
use terminal_interpreter::TerminalPlacedViewEstablishment;
use terminal_verifier::AcceptedControlCycle;

/// Native-admitted input and the provider establishments bound to its
/// direct-entry placed-view roster rows. Construction is private:
/// `AdmittedNativeArtifact::try_into_native_input` joins one supply per
/// declared row, so an empty roster takes an empty supply.
#[derive(Debug, Clone)]
pub struct VerifiedNativeArtifactInput {
    optimization_input: VerifiedPsiOptimizationInput,
    placed_view_establishments: Vec<TerminalPlacedViewEstablishment>,
    accepted_control_cycles: Vec<AcceptedControlCycle>,
}

impl VerifiedNativeArtifactInput {
    pub fn plan(&self) -> &AbstractOperationPlan {
        self.optimization_input.plan()
    }

    pub const fn context(&self) -> &VerifiedPsiOptimizationContext {
        self.optimization_input.context()
    }

    /// The provider establishments bound to this input's direct-entry roster
    /// rows, in roster order. Empty for a program that declared none.
    pub fn placed_view_establishments(&self) -> &[TerminalPlacedViewEstablishment] {
        &self.placed_view_establishments
    }

    /// The ranked control cycles the verifier accepted for this exact
    /// artifact, in the verifier's acceptance order. Native admission retains
    /// them so a realization consumer can observe which cycles were admitted
    /// rather than re-deriving them from the retained module and bundle.
    pub fn accepted_control_cycles(&self) -> &[AcceptedControlCycle] {
        &self.accepted_control_cycles
    }

    /// Establishment bindings are invocation-scoped executable evidence and
    /// do not transfer into optimizer authority; the provider keeps custody
    /// of its supply.
    pub fn into_optimization_input(self) -> VerifiedPsiOptimizationInput {
        self.optimization_input
    }
}

/// One canonical native-admitted plan with its exact retained verifier context.
/// Placed-view rows are semantic custody, not backing or access-event authority.
#[derive(Debug, Clone)]
pub struct AdmittedNativeArtifact {
    optimization_input: VerifiedPsiOptimizationInput,
    accepted_control_cycles: Vec<AcceptedControlCycle>,
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

    /// The ranked control cycles the verifier accepted for this exact
    /// artifact, in the verifier's acceptance order. The roster is admission
    /// authority bound to this artifact; it stays available through
    /// `try_into_native_input` for the realization boundary but does not
    /// transfer into optimizer authority via `into_optimization_artifact`.
    pub fn accepted_control_cycles(&self) -> &[AcceptedControlCycle] {
        &self.accepted_control_cycles
    }

    /// Downgrade authority while preserving the complete semantic roster.
    pub fn into_optimization_artifact(self) -> AdmittedOptimizationArtifact {
        AdmittedOptimizationArtifact {
            input: self.optimization_input,
        }
    }

    /// The lowered plan alone. The plan carries no placed-view rows or
    /// verifier context; consumers that need either keep the admission and
    /// read `placed_view_inputs` or `context` instead.
    pub fn into_plan(self) -> AbstractOperationPlan {
        self.optimization_input.plan
    }

    /// The executable-image realization input: join one provider
    /// establishment to each placed-view roster row the direct entry
    /// declares. A program that declared none takes an empty supply. The
    /// bound supplies ride inside the native input so the realized entry
    /// boundary can lend the exact qualified backing for the invocation's
    /// duration; a supply that answers no declared row, answers one twice,
    /// carries noncanonical qualifications, or overlaps another exclusive
    /// referent rejects here rather than at access, and a row left
    /// unanswered still fails custody — a roster or a pointer is not the
    /// authority that lends the referent. Native execution of the derived
    /// placed-entry ABI with a lent host referent is exercised by the
    /// compiler control `direct_placed_view_input_survives_codec_and_native_replay`.
    pub fn try_into_native_input(
        self,
        establishments: &[TerminalPlacedViewEstablishment],
    ) -> Result<VerifiedNativeArtifactInput, ArtifactLoweringError> {
        let placed_view_establishments = super::establishment::establish_native_placed_view_inputs(
            self.context().module(),
            establishments,
        )?;
        Ok(VerifiedNativeArtifactInput {
            optimization_input: self.optimization_input,
            placed_view_establishments,
            accepted_control_cycles: self.accepted_control_cycles,
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
    // The accepted roster is captured from the native-verified module because
    // the optimizer-eligibility transfer below does not re-expose it.
    let accepted_control_cycles = verified.accepted_control_cycles().to_vec();
    let optimizable = verified
        .into_optimization()
        .map_err(ArtifactLoweringError::Verification)?;
    let context = retain_verified_optimization_context(&optimizable)?;
    Ok(AdmittedNativeArtifact {
        optimization_input: VerifiedPsiOptimizationInput { plan, context },
        accepted_control_cycles,
    })
}
