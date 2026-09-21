//! Optimizer module role: executable entrance. Canonical preparation followed
//! by one native admission. The result retains its exact placed-view input
//! roster and downgrades to optimizer authority on request; optimizer
//! authority never promotes back.

mod error;
mod establishment;
mod native;
mod retention;

pub use error::ArtifactLoweringError;
pub use establishment::TerminalPlacedViewEstablishment;
pub use native::{AdmittedNativeArtifact, VerifiedNativeArtifactInput};
pub use retention::AdmittedOptimizationArtifact;

/// Canonical artifact sections. An offered ledger must replay exactly before
/// proof decoding; absence does not bypass ordinary verification. Admission
/// requires a subject-sealed proof section naming this module's reconstructed
/// identity.
#[derive(Clone, Copy)]
pub struct ArtifactSections<'artifact> {
    pub semantic_bytes: &'artifact [u8],
    pub proof_bytes: &'artifact [u8],
    pub obligation_ledger_bytes: Option<&'artifact [u8]>,
}

/// Decode, optionally replay the ledger, verify ordinary native authority,
/// lower, and retain optimizer eligibility beside the lowered plan. This is
/// the one artifact entrance: an optimizer consumer downgrades the result
/// through `AdmittedNativeArtifact::into_optimization_artifact`, and a
/// realization consumer binds its placed-view establishments through
/// `AdmittedNativeArtifact::try_into_native_input`.
pub fn lower_artifact(
    sections: ArtifactSections<'_>,
    profile: &proof_admission::AdmissionProfile,
) -> Result<AdmittedNativeArtifact, ArtifactLoweringError> {
    let (module, proof) = prepare_artifact(sections)?;
    native::lower_decoded_native_module(&module, &proof, profile)
}

/// Decode the module, replay any offered obligation ledger, and decode the
/// subject-sealed proof section: the seal must name this module's
/// reconstructed identity, so a bare proof bundle or a section sealed to
/// another subject is rejected here.
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
    let proof = terminal_codec::decode_proof_section_for(&module, sections.proof_bytes)
        .map_err(ArtifactLoweringError::ProofDecode)?;
    Ok((module, proof))
}
