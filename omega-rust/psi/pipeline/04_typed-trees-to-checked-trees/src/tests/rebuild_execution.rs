//! Rebuild plans after a test deliberately tampers with checked evidence.
//! Production finalizes execution inside checking; provider selection must
//! not re-enter that phase. Mutation controls use the same private builders
//! to verify that inconsistent evidence cannot recover an executable plan.

use crate::execution::execution_plans::{ExecutionPlans, build_execution_plans};
use checked_trees::CheckedTrees;

/// Rebuild the three plan lanes together; failed mutation checks publish nothing.
pub(crate) fn rebuild(
    mut checked: CheckedTrees,
) -> Result<CheckedTrees, Vec<diagnostics::Diagnostic>> {
    let call_frames = validation::CallFrameResolver::new(&checked.typed);
    let ExecutionPlans {
        boundary_returns,
        unit_effects,
        structural_scalar_returns,
        cleanup_diagnostics,
    } = build_execution_plans(&checked.typed, &checked.facts, call_frames.as_ref());
    if !cleanup_diagnostics.is_empty() {
        return Err(cleanup_diagnostics);
    }
    checked.facts.flow.terminal_boundary_scalar_returns = boundary_returns;
    checked.facts.flow.terminal_unit_effects = unit_effects;
    checked.facts.flow.terminal_structural_scalar_returns = structural_scalar_returns;
    Ok(checked)
}
