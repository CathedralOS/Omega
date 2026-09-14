//! Optimizer module role: executable entrance. Canonical preparation followed
//! by distinct ordinary, optimizer, or native
//! admission. Every result retains its exact placed-view input roster.

mod error;
mod native;
mod retention;

pub use error::ArtifactLoweringError;
pub use native::{AdmittedNativeArtifact, VerifiedNativeArtifactInput};
pub use retention::AdmittedOptimizationArtifact;

use abstract_operations::{AbstractOperationPlan, AbstractOperationPlanWithPlacedViewInputs};
use retention::retain_verified_optimization_input;

/// Canonical artifact sections. An offered ledger must replay exactly before
/// proof decoding; absence does not bypass ordinary verification.
#[derive(Clone, Copy)]
pub struct ArtifactSections<'artifact> {
    pub semantic_bytes: &'artifact [u8],
    pub proof_bytes: &'artifact [u8],
    pub obligation_ledger_bytes: Option<&'artifact [u8]>,
}

/// An ordinary admitted plan and its exact semantic placed-view custody.
#[derive(Debug, Clone)]
pub struct AdmittedArtifactPlan {
    parts: AbstractOperationPlanWithPlacedViewInputs,
}

impl AdmittedArtifactPlan {
    pub fn plan(&self) -> &AbstractOperationPlan {
        &self.parts.plan
    }

    pub fn placed_view_inputs(&self) -> &[terminal_psi::TerminalPlacedViewInput] {
        &self.parts.placed_view_inputs
    }

    /// Transfer the complete roster to a custody-aware downstream consumer.
    pub fn into_parts(self) -> AbstractOperationPlanWithPlacedViewInputs {
        self.parts
    }

    /// Consumers without placed-view custody support may only take empty rosters.
    pub fn try_into_plan(self) -> Result<AbstractOperationPlan, ArtifactLoweringError> {
        require_empty_placed_view_inputs(self.placed_view_inputs())?;
        Ok(self.parts.plan)
    }
}

/// Decode, optionally replay the ledger, verify ordinary authority, and lower.
pub fn lower_artifact(
    sections: ArtifactSections<'_>,
    profile: &proof_admission::AdmissionProfile,
) -> Result<AdmittedArtifactPlan, ArtifactLoweringError> {
    let (module, proof) = prepare_artifact(sections)?;
    let verified = terminal_verifier::verify_module(&module, &proof, profile)
        .map_err(ArtifactLoweringError::Verification)?;
    let plan = crate::lowering::lower_decoded_verified_module(&verified)
        .map_err(ArtifactLoweringError::Lowering)?;
    Ok(AdmittedArtifactPlan {
        parts: AbstractOperationPlanWithPlacedViewInputs {
            plan,
            placed_view_inputs: module.placed_view_inputs,
        },
    })
}

/// Admit optimizer authority, which cannot be promoted to native authority.
pub fn lower_artifact_for_optimization(
    sections: ArtifactSections<'_>,
    profile: &proof_admission::AdmissionProfile,
) -> Result<AdmittedOptimizationArtifact, ArtifactLoweringError> {
    let (module, proof) = prepare_artifact(sections)?;
    let verified = terminal_verifier::verify_module_for_optimization(&module, &proof, profile)
        .map_err(ArtifactLoweringError::Verification)?;
    Ok(AdmittedOptimizationArtifact {
        input: retain_verified_optimization_input(&verified)?,
    })
}

/// Admit ordinary native authority before retaining optimizer eligibility.
/// Legacy countdown input rejects rather than falling back to another authority.
pub fn lower_artifact_for_native_realization(
    sections: ArtifactSections<'_>,
    profile: &proof_admission::AdmissionProfile,
) -> Result<AdmittedNativeArtifact, ArtifactLoweringError> {
    let (module, proof) = prepare_artifact(sections)?;
    native::lower_decoded_native_module(&module, &proof, profile)
}

fn prepare_artifact(
    sections: ArtifactSections<'_>,
) -> Result<(terminal_psi::TerminalModule, terminal_verifier::ProofBundle), ArtifactLoweringError> {
    let module = terminal_codec::decode_module(sections.semantic_bytes)
        .map_err(ArtifactLoweringError::SemanticDecode)?;
    if let Some(ledger_bytes) = sections.obligation_ledger_bytes {
        let ledger = terminal_codec::decode_terminal_obligation_ledger(ledger_bytes)
            .map_err(ArtifactLoweringError::ObligationLedgerDecode)?;
        let trust_graph = terminal_codec::current_terminal_trust_graph()
            .map_err(ArtifactLoweringError::TrustGraph)?;
        terminal_codec::validate_terminal_obligation_ledger(&ledger, &module, &trust_graph)
            .map_err(ArtifactLoweringError::ObligationReplay)?;
    }
    let proof = terminal_codec::decode_proof_bundle(sections.proof_bytes)
        .map_err(ArtifactLoweringError::ProofDecode)?;
    Ok((module, proof))
}

fn require_empty_placed_view_inputs(
    inputs: &[terminal_psi::TerminalPlacedViewInput],
) -> Result<(), ArtifactLoweringError> {
    if inputs.is_empty() {
        Ok(())
    } else {
        Err(ArtifactLoweringError::PlacedViewInputsRequireCustodyLowering)
    }
}
