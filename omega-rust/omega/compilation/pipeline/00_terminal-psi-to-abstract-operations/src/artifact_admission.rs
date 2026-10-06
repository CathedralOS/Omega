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
    // Loose section bytes carry no decoded module, so this is the one place
    // that runs canonical module decoding for them. A consumer holding the
    // whole `CanonicalTerminalArtifact` reads the artifact's own decoded
    // module instead and reaches the identical admission below.
    let module = terminal_codec::decode_module(sections.semantic_bytes)
        .map_err(ArtifactLoweringError::SemanticDecode)?;
    lower_decoded_artifact(
        &module,
        sections.proof_bytes,
        sections.obligation_ledger_bytes,
        profile,
    )
}

/// Replay any offered obligation ledger against this exact decoded module,
/// decode the subject-sealed proof section, verify ordinary native authority,
/// and lower. The proof seal must name this module's reconstructed identity,
/// so a bare proof bundle or a section sealed to another subject is rejected
/// here.
///
/// `module` must be the canonical decoding of the artifact's semantic bytes —
/// the only two producers of such a module are `terminal_codec::decode_module`
/// and `CanonicalTerminalArtifact::semantic_module`, which is that same decode
/// retained. Admission therefore replays identically whichever caller arrives.
pub(crate) fn lower_decoded_artifact(
    module: &terminal_psi::TerminalModule,
    proof_bytes: &[u8],
    obligation_ledger_bytes: Option<&[u8]>,
    profile: &proof_admission::AdmissionProfile,
) -> Result<AdmittedNativeArtifact, ArtifactLoweringError> {
    // Proof-carrying admission is retired: the artifact carries no proof
    // to check, so admission validates the module's structure and lowers it.
    let _ = (proof_bytes, obligation_ledger_bytes, profile);
    native::lower_decoded_native_module(module)
}
