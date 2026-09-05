#![forbid(unsafe_code)]

//! Canonical publication of validated, optimized Psi.
//!
//! This stage seals source-free semantics and their proof and debug companions.
//! It accepts only the output of the explicit pre-Terminal optimization stage.

use lowered_psi_to_lowered_psi::PsiOptimizationStageResult;

/// Produce the canonical source-free artifact consumed at the Psi/Omega seam.
///
/// Unsupported checked constructs fail at lowering; there is no alternate
/// checked-tree backend to select as a fallback.
pub fn finalize_terminal_artifact(
    optimized: &PsiOptimizationStageResult,
) -> Result<terminal_codec::CanonicalTerminalArtifact, terminal_codec::CanonicalTerminalArtifactError>
{
    let lowered = optimized.lowered();
    terminal_codec::CanonicalTerminalArtifact::from_parts(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        optimized.execution(),
        lowered.debug_map.as_ref(),
    )
}

mod source_custody;
pub use source_custody::{
    CheckedBoundaryOperatorApplicationOccurrence, CheckedBoundaryOperatorApplicationScope,
    checked_boundary_operator_scope,
};
