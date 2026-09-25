//! Replay the canonical semantic and proof sections of one Terminal artifact
//! under the request admission profile.

use diagnostics::Diagnostic;

pub(crate) fn verify_terminal_artifact(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    profile: &proof_admission::AdmissionProfile,
) -> Result<(), Vec<Diagnostic>> {
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).map_err(|error| {
        vec![Diagnostic::error(format!(
            "terminal-artifact verification could not decode canonical semantics: {error}"
        ))]
    })?;
    // Verification consumes only the subject-sealed proof section: the seal
    // must name the identity reconstructed from this artifact's own semantic
    // section, so a proof sealed for another subject cannot be replayed here.
    let proof = terminal_codec::decode_proof_section_for(&module, artifact.proof_bytes()).map_err(
        |error| {
            vec![Diagnostic::error(format!(
                "terminal-artifact verification could not decode canonical proof: {error}"
            ))]
        },
    )?;
    terminal_verifier::verify_module(&module, &proof, profile)
        .map(|_| ())
        .map_err(|error| {
            vec![Diagnostic::error(format!(
                "terminal-artifact verification failed: {error}"
            ))]
        })
}
