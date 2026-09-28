use lowered_psi_to_lowered_psi::PsiOptimizationStageResult;

/// Produce the canonical source-free artifact consumed at the Psi/Omega seam.
///
/// Unsupported checked constructs fail at lowering; there is no alternate
/// checked-tree backend to select as a fallback. The artifact carries no
/// mathematical-signature section; a checked program with mathematical
/// declarations publishes through
/// [`finalize_terminal_artifact_with_mathematical_signature`].
pub fn finalize_terminal_artifact(
    optimized: &PsiOptimizationStageResult,
) -> Result<terminal_codec::CanonicalTerminalArtifact, terminal_codec::CanonicalTerminalArtifactError>
{
    finalize_terminal_artifact_with_mathematical_signature(optimized, None)
}

/// Produce the canonical artifact beside the checked program's mathematical
/// signature evidence.
///
/// `mathematical_signature` is `ProofFacts::mathematical_signature` — `Some`
/// exactly when the checked cohort carried `let`/`boundary let`
/// mathematical declarations. The checked kernel signature is encoded onto
/// the canonical `PSISIG` wire here, at the publication boundary, so the
/// artifact carries the signature the receiver's kernel re-decides; the
/// wire never stores a producer verdict. An encode refusal (a resource
/// bound, never a judgment) surfaces as
/// [`terminal_codec::CanonicalTerminalArtifactError::MathematicalSignature`].
pub fn finalize_terminal_artifact_with_mathematical_signature(
    optimized: &PsiOptimizationStageResult,
    mathematical_signature: Option<&checked_trees::CheckedMathematicalSignature>,
) -> Result<terminal_codec::CanonicalTerminalArtifact, terminal_codec::CanonicalTerminalArtifactError>
{
    let lowered = optimized.lowered();
    let signature_bytes = mathematical_signature
        .map(|signature| {
            // `Signature` shares its declaration list through `Rc`, which the
            // checked facts cannot hold without losing `Send`; the codec
            // producer rebuilds it from the plain vector the facts retained.
            let signature_value =
                proof_admission::Signature::from_declarations(signature.declarations.clone());
            terminal_codec::encode_mathematical_signature(&signature.arena, &signature_value)
        })
        .transpose()
        .map_err(terminal_codec::CanonicalTerminalArtifactError::MathematicalSignature)?;
    terminal_codec::CanonicalTerminalArtifact::from_parts_with_mathematical_signature(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        optimized.execution(),
        lowered.debug_map.as_ref(),
        signature_bytes.as_deref(),
    )
}
