use lowered_psi_to_lowered_psi::PsiOptimizationStageResult;

/// Produce the canonical source-free artifact consumed at the Psi/Omega seam.
///
/// Unsupported checked constructs fail at lowering; there is no alternate
/// checked-tree backend to select as a fallback.
pub fn finalize_terminal_artifact(
    optimized: PsiOptimizationStageResult,
) -> Result<
    (
        terminal_codec::CanonicalTerminalArtifact,
        lowered_psi::LoweredSourceCustody,
    ),
    terminal_codec::CanonicalTerminalArtifactError,
> {
    let (lowered, execution) = optimized.into_parts();
    let (module, debug_map, source_custody) = lowered.into_parts();
    let artifact = terminal_codec::CanonicalTerminalArtifact::from_parts(
        module,
        execution,
        debug_map.as_ref(),
    )?;
    Ok((artifact, source_custody))
}
