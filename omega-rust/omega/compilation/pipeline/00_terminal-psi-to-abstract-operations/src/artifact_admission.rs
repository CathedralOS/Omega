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

/// Canonical artifact sections.
#[derive(Clone, Copy)]
pub struct ArtifactSections<'artifact> {
    pub semantic_bytes: &'artifact [u8],
}

/// Decode, validate the module's structure, lower, and retain optimizer
/// eligibility beside the lowered plan. This is
/// the one artifact entrance: an optimizer consumer downgrades the result
/// through `AdmittedNativeArtifact::into_optimization_artifact`, and a
/// realization consumer binds its placed-view establishments through
/// `AdmittedNativeArtifact::try_into_native_input`.
pub fn lower_artifact(sections: ArtifactSections<'_>) -> Result<AdmittedNativeArtifact, ArtifactLoweringError> {
    // Loose section bytes carry no decoded module, so this is the one place
    // that runs canonical module decoding for them. A consumer holding the
    // whole `CanonicalTerminalArtifact` reads the artifact's own decoded
    // module instead and reaches the identical admission below.
    let module = terminal_codec::decode_module(sections.semantic_bytes)
        .map_err(ArtifactLoweringError::SemanticDecode)?;
    lower_decoded_artifact(&module)
}

/// Validate and lower one decoded canonical module.
pub(crate) fn lower_decoded_artifact(
    module: &terminal_psi::TerminalModule,
) -> Result<AdmittedNativeArtifact, ArtifactLoweringError> {
    native::lower_decoded_native_module(module)
}
